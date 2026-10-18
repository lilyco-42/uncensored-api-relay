# lain42.top 接入实测结论

## 现状（2026-10-18 实测）

| 目标 | 结果 |
|---|---|
| https://lain42.top/ | 200，"云枢智创 · lain42"，提供 Rembg Studio / 算力 / AI API / 代理订阅 |
| https://lain42.top/api/* | 200，返回 SPA 页面（Vue + /panel/favicon.svg）→ 说明有前端面板 |
| https://lain42.top/v1/models | 404 → 站点根路径**未**直接提供 OpenAI 兼容接口 |
| http://lain42.top:3000/api/status | **502 Bad Gateway** |
| http://lain42.top:3000/v1/models | **502 Bad Gateway** |

DNS 解析到 198.18.0.38（fake-ip 段，典型 Clash/Sing-box TUN 模式特征），
所以本机访问 lain42.top 实际走了本地代理规则。

## 关键发现：cc-switch 里已经配了 lain42.top

本机 cc-switch 数据库（~/.cc-switch/cc-switch.db）中存在 3 个指向
`http://lain42.top:3000` 的 NewAPI Provider（各 app_type 一份，共用同一个 key）：

* claude   → `ANTHROPIC_BASE_URL=http://lain42.top:3000`，模型 claude-sonnet-5
* codex    → `base_url=http://lain42.top:3000/v1`，wire_api=responses，模型 gpt-5.6-sol
* gemini   → `GOOGLE_GEMINI_BASE_URL=http://lain42.top:3000`，模型 gemini-3.6-flash

即：**lain42.top:3000 是一个 NewAPI/one-api 类的中转面板**，路径约定为
`{base}/v1/chat/completions`（OpenAI 协议）与 `{base}/v1/messages`（Anthropic 协议）。
目前返回 502，面板后端未启动或正在重启。

## 因此有两种接入姿势

### 姿势 A：lain42.top:3000 作为 Rust 中转站的上游（推荐）

面板恢复后，用 Rust 中转站给面板前面再加一层无审查模型路由：

```powershell
$env:UPSTREAM_URL = "http://lain42.top:3000/v1"
$env:MODEL_MAP    = "gpt-4=uncensored-27b,claude-sonnet-4=uncensored-27b"
$env:BIND_ADDR    = "127.0.0.1:8080"
.\uncensored-proxy.exe
```

然后 cc-switch 的 `ANTHROPIC_BASE_URL` 指向 `http://127.0.0.1:8080`。

### 姿势 B：Rust 中转站直接对公网提供 lain42.top/v1

把 deploy/ 下的反代配置接到 lain42.top 的公网入口（见 README 第六节），
此时 `UPSTREAM_URL` 指向真正的无审查模型后端（Kaggle TPU 隧道等）。

## 注意

cc-switch 里已有的 lain42.top key 形如 `sk-HkVuVY...`（只在本机 DB 出现），
若把 Rust 中转站部署到公网，**必须**换一个新 key 并在反代层加鉴权，
不要把该 key 提交到任何仓库。
