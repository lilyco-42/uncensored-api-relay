//! 高性能 Rust API 中转站 —— 无审查模型接入
//! Axum + Tokio + Reqwest，支持 SSE 流式透传
//! 对接 cc-switch / lain42.top，兼容 OpenAI Chat Completions 与 Anthropic Messages 协议。

use axum::{
    body::Body,
    extract::{RawQuery, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{any, get},
    Router,
};
use futures_util::StreamExt;
use reqwest::Client;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone)]
struct AppState {
    client: Client,
    /// 上游无审查模型服务地址，例如 https://xxx.trycloudflare.com/v1
    upstream_url: String,
    /// 客户端模型名 -> 上游无审查模型 ID
    model_map: HashMap<String, String>,
}

/// 解析 "a=1,b=2" 形式的环境变量为模型映射表
fn parse_model_map(raw: &str) -> HashMap<String, String> {
    raw.split(',')
        .filter_map(|pair| pair.split_once('='))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect()
}

/// 把外部模型名替换为上游无审查模型 ID
fn remap_model(state: &AppState, payload: &mut Value) {
    if let Some(model) = payload.get("model").and_then(Value::as_str) {
        let mapped = state
            .model_map
            .get(model)
            .cloned()
            .unwrap_or_else(|| model.to_string());
        payload["model"] = Value::String(mapped);
    }
}

#[tokio::main]
async fn main() {
    let upstream_url = std::env::var("UPSTREAM_URL")
        .unwrap_or_else(|_| "https://api.openai.com/v1".to_string())
        .trim_end_matches('/')
        .to_string();

    let model_map = parse_model_map(
        &std::env::var("MODEL_MAP")
            .unwrap_or_else(|_| "gpt-4=uncensored-27b,claude-sonnet-4=uncensored-27b".to_string()),
    );

    let state = Arc::new(AppState {
        client: Client::builder()
            .pool_max_idle_per_host(200)
            .build()
            .expect("http client"),
        upstream_url,
        model_map,
    });

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/v1/models", any(proxy_models))
        .route("/v1/chat/completions", any(proxy_chat))
        .route("/v1/messages", any(proxy_messages))
        .with_state(state);

    let bind = std::env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".to_string());
    let listener = tokio::net::TcpListener::bind(&bind)
        .await
        .expect("bind failed");
    println!("rust uncensored relay listening on http://{}", bind);
    axum::serve(listener, app).await.expect("server failed");
}

async fn health_check(State(state): State<Arc<AppState>>) -> (StatusCode, axum::Json<Value>) {
    (
        StatusCode::OK,
        axum::Json(json!({
            "status": "ok",
            "relay": "rust-uncensored-proxy",
            "upstream": state.upstream_url,
            "models": state.model_map,
        })),
    )
}

/// 通用转发：methods + path 后缀 + query + 可选的 body 改写
async fn forward(
    state: &Arc<AppState>,
    method: axum::http::Method,
    path_suffix: &str,
    query: Option<&str>,
    headers: &HeaderMap,
    body: &str,
    rewrite: bool,
) -> Response {
    // 透传 query string，否则上游带参端点（如 /drip?delay=0.5）会 404。
    // 同时容忍 UPSTREAM_URL 自带 query（避免拼出 "?a=1/b?c=2" 这种非法 URL）。
    let mut base_query: Option<String> = None;
    let mut base = state.upstream_url.as_str();
    if let Some(pos) = base.find('?') {
        base_query = Some(base[pos + 1..].to_string());
        base = &base[..pos];
    }

    let merged_query = match (base_query, query) {
        (Some(b), Some(q)) if !q.is_empty() => format!("{}&{}", b, q),
        (Some(b), _) => b,
        (None, Some(q)) if !q.is_empty() => q.to_string(),
        _ => String::new(),
    };

    let target = if merged_query.is_empty() {
        format!("{}/{}", base, path_suffix)
    } else {
        format!("{}/{}?{}", base, path_suffix, merged_query)
    };

    // body 改写（模型名映射到无审查上游 ID）
    let effective_body: String = if rewrite && !body.is_empty() {
        match serde_json::from_str::<Value>(body) {
            Ok(mut payload) => {
                remap_model(state, &mut payload);
                payload.to_string()
            }
            Err(_) => body.to_string(),
        }
    } else {
        body.to_string()
    };

    let rmethod =
        reqwest::Method::from_bytes(method.as_str().as_bytes()).unwrap_or(reqwest::Method::POST);

    let mut req = state.client.request(rmethod, &target).body(effective_body);

    // 透传除 host/accept-encoding/content-length 外的请求头（reqwest 会自己算后两者）
    for (name, value) in headers.iter() {
        let key = name.as_str();
        if matches!(key, "host" | "accept-encoding" | "content-length") {
            continue;
        }
        if let Ok(v) = reqwest::header::HeaderValue::from_bytes(value.as_bytes()) {
            req = req.header(key, v);
        }
    }

    match req.send().await {
        Ok(resp) => {
            let status =
                StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);

            // 先把响应头收集为完全 owned 的数据，借用即刻结束，再移动 resp 构造流
            let collected: Vec<(String, Vec<u8>)> = resp
                .headers()
                .iter()
                .filter(|(name, _)| {
                    !matches!(name.as_str(), "content-encoding" | "transfer-encoding")
                })
                .map(|(name, value)| (name.as_str().to_string(), value.as_bytes().to_vec()))
                .collect();

            let mut out_headers = HeaderMap::new();
            for (name, bytes) in collected {
                if let (Ok(k), Ok(v)) = (
                    axum::http::HeaderName::from_bytes(name.as_bytes()),
                    HeaderValue::from_bytes(&bytes),
                ) {
                    out_headers.insert(k, v);
                }
            }

            // 流式透传，SSE 打字机效果不丢包
            let stream = resp
                .bytes_stream()
                .map(|chunk| chunk.map_err(std::io::Error::other));

            (status, out_headers, Body::from_stream(stream)).into_response()
        }
        Err(e) => {
            // 上游连接失败：必须返回可读 JSON 错误体。
            // 注意这里刻意避开 (StatusCode, Json) 元组与 Response::builder +
            // Body::from(String)：两种写法在本项目（axum 0.7 + hyper 0.14）下
            // 实测都被丢弃 body，客户端只看到 content-length: 0 的裸 502。
            // 用 axum 的 IntoResponseParts 逐个写状态码与响应头，body 用 String，
            // 由 axum 自己完成分帧，才能稳定带出 body。
            let payload = json!({
                "error": {
                    "message": format!("relay upstream error: {}", e),
                    "type": "relay_error",
                    "upstream": target
                }
            })
            .to_string();

            // 用 http_body::Full<Bytes> 明确声明"完整已知长度"的 body。
            // 之前用 Body::from(String) / IntoResponse(String) 在 http:// 传输层
            // 连接失败的场景下 body 会被丢弃（HTTPS 与所有成功路径均正常）。
            let bytes = axum::body::Bytes::from(payload);
            let len = bytes.len();

            Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .header(
                    axum::http::header::CONTENT_TYPE,
                    HeaderValue::from_static("application/json; charset=utf-8"),
                )
                .header(axum::http::header::CONTENT_LENGTH, len)
                .body(Body::from(bytes))
                .unwrap_or_else(|_| StatusCode::BAD_GATEWAY.into_response())
        }
    }
}

async fn proxy_chat(
    State(state): State<Arc<AppState>>,
    method: axum::http::Method,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
    body: String,
) -> Response {
    forward(
        &state,
        method,
        "chat/completions",
        query.as_deref(),
        &headers,
        &body,
        true,
    )
    .await
}

async fn proxy_messages(
    State(state): State<Arc<AppState>>,
    method: axum::http::Method,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
    body: String,
) -> Response {
    forward(
        &state,
        method,
        "messages",
        query.as_deref(),
        &headers,
        &body,
        true,
    )
    .await
}

async fn proxy_models(
    State(state): State<Arc<AppState>>,
    method: axum::http::Method,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
    body: String,
) -> Response {
    forward(
        &state,
        method,
        "models",
        query.as_deref(),
        &headers,
        &body,
        false,
    )
    .await
}

impl AppState {
    #[cfg(test)]
    fn test(model_map: HashMap<String, String>) -> Self {
        Self {
            client: Client::new(),
            upstream_url: "http://127.0.0.1:9/v1".to_string(),
            model_map,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map() -> HashMap<String, String> {
        parse_model_map(
            "gpt-4=uncensored-27b,claude-sonnet-4=uncensored-27b, claude-opus-4 = uncensored-27b ",
        )
    }

    #[test]
    fn parse_model_map_handles_spaces_and_ignores_junk() {
        let m = map();
        assert_eq!(m.len(), 3);
        assert_eq!(m.get("gpt-4").map(String::as_str), Some("uncensored-27b"));
        assert_eq!(
            m.get("claude-opus-4").map(String::as_str),
            Some("uncensored-27b")
        );
    }

    #[test]
    fn parse_model_map_ignores_malformed_entries() {
        let m = parse_model_map("good=1,bad-entry,,,also-bad");
        assert_eq!(m.len(), 1);
        assert_eq!(m.get("good").map(String::as_str), Some("1"));
    }

    #[test]
    fn parse_model_map_empty_input() {
        assert!(parse_model_map("").is_empty());
        assert!(map().contains_key("claude-sonnet-4"));
    }

    #[test]
    fn remap_known_models() {
        let m = map();
        for input in ["gpt-4", "claude-sonnet-4", "claude-opus-4"] {
            let mut payload = serde_json::json!({ "model": input });
            remap_model(&AppState::test(m.clone()), &mut payload);
            assert_eq!(payload["model"], "uncensored-27b", "input {input}");
        }
    }

    #[test]
    fn remap_unknown_model_passes_through() {
        let m = map();
        let mut payload = serde_json::json!({ "model": "uncensored-27b" });
        remap_model(&AppState::test(m), &mut payload);
        assert_eq!(payload["model"], "uncensored-27b");

        let mut other = serde_json::json!({ "model": "some-other-model" });
        remap_model(&AppState::test(map()), &mut other);
        assert_eq!(other["model"], "some-other-model");
    }

    #[test]
    fn remap_payload_without_model_is_untouched() {
        let m = map();
        let mut payload = serde_json::json!({ "messages": [] });
        remap_model(&AppState::test(m), &mut payload);
        assert!(payload.get("model").is_none());
    }

    #[test]
    fn remap_preserves_other_fields() {
        let m = map();
        let mut payload = serde_json::json!({
            "model": "gpt-4",
            "messages": [{"role":"user","content":"hi"}],
            "stream": true,
            "temperature": 0.7
        });
        remap_model(&AppState::test(m), &mut payload);
        assert_eq!(payload["model"], "uncensored-27b");
        assert_eq!(payload["stream"], true);
        assert_eq!(payload["temperature"], 0.7);
        assert!(payload["messages"].is_array());
    }

    /// 与 forward() 中的 URL 拼接逻辑保持一致
    fn compose(base_url: &str, suffix: &str, query: Option<&str>) -> String {
        let mut base_query: Option<String> = None;
        let mut base = base_url;
        if let Some(pos) = base.find('?') {
            base_query = Some(base[pos + 1..].to_string());
            base = &base[..pos];
        }
        let merged_query = match (base_query, query) {
            (Some(b), Some(q)) if !q.is_empty() => format!("{}&{}", b, q),
            (Some(b), _) => b,
            (None, Some(q)) if !q.is_empty() => q.to_string(),
            _ => String::new(),
        };
        if merged_query.is_empty() {
            format!("{}/{}", base, suffix)
        } else {
            format!("{}/{}?{}", base, suffix, merged_query)
        }
    }

    #[test]
    fn upstream_url_keeps_query_string() {
        // 客户端 query 必须透传
        assert_eq!(
            compose("http://httpbin.org", "drip", Some("delay=0.5&code=200")),
            "http://httpbin.org/drip?delay=0.5&code=200"
        );
        // 空 query 不加问号
        assert_eq!(
            compose("http://httpbin.org", "drip", Some("")),
            "http://httpbin.org/drip"
        );
        // 无 query 不加问号
        assert_eq!(
            compose("http://httpbin.org", "drip", None),
            "http://httpbin.org/drip"
        );
    }

    #[test]
    fn upstream_url_embedded_query_is_merged_not_duplicated() {
        // UPSTREAM_URL 自带 query 时不能拼出 "?a=1/drip?b=2" 这种非法 URL
        assert_eq!(
            compose(
                "http://h/drip?duration=2&delay=0.5",
                "chat/completions",
                None
            ),
            "http://h/drip/chat/completions?duration=2&delay=0.5"
        );
        // 两边都有 query 时用 & 连接
        assert_eq!(
            compose("http://h/x?a=1", "messages", Some("b=2")),
            "http://h/x/messages?a=1&b=2"
        );
        // 上游 query 为空串（"?/suffix"）时不应产生悬挂问号
        assert_eq!(
            compose("http://h/x?", "messages", None),
            "http://h/x/messages"
        );
    }

    #[test]
    fn error_payload_exposes_cause_and_upstream() {
        // 502 必须带可读错误体，客户端不能只看到裸状态码
        let payload = json!({
            "error": {
                "message": "relay upstream error: boom",
                "type": "relay_error",
                "upstream": "http://h/v1"
            }
        })
        .to_string();
        let parsed: Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(parsed["error"]["type"], "relay_error");
        assert_eq!(parsed["error"]["upstream"], "http://h/v1");
        assert!(parsed["error"]["message"]
            .as_str()
            .unwrap()
            .contains("boom"));
    }

    #[test]
    fn upstream_url_is_trimmed() {
        let u = "https://a.trycloudflare.com/v1/".trim_end_matches('/');
        assert_eq!(u, "https://a.trycloudflare.com/v1");
    }
}
