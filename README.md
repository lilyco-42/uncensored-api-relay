# uncensored-proxy

高性能 Rust API 中转站 —— 把本机 cc-switch / lain42.top 接入无审查（Uncensored）大模型。

| 项目 | 说明 |
|---|---|
| 技术栈 | Axum 0.7 + Tokio + Reqwest |
| 定位 | 让 cc-switch / lain42.top 走无审查 27B 模型的中转网关 |
| 协议 | OpenAI Chat Completions + Anthropic Messages + /v1/models |
| 流式 | SSE 字节级透传，打字机效果零丢包 |
| 交付物 | CI 产出 Windows / Linux x64 / Linux arm64 三个 release 二进制 |
| 验证 | GitHub Actions：fmt + clippy(-D warnings) + 8 个单测，全绿才发构建 |

> 本机**不编译、不跑测试**。构建与验证只走 `.github/workflows/ci.yml`。
> 运行用的二进制请从 CI Artifacts 或 Release 下载。

## 一、构建与验证：只走 GitHub Actions

**本机不编译、不跑测试**。所有构建、格式化、lint、测试与交叉编译发布
全部由 `.github/workflows/ci.yml` 在 GitHub Actions 上完成。

```bash
git init && git add -A && git commit -m "feat: rust uncensored api relay"
git branch -M main
git remote add origin https://github.com/<你的账号>/uncensored-api-relay.git
git push -u origin main
```

推送后 Actions 自动执行，三个 job：

| Job | 内容 | 门槛 |
|---|---|---|
| `test` | `cargo fmt --check` → `cargo clippy -D warnings` → `cargo test` | 8 个单测全过 |
| `build` | 三平台交叉编译 release | Windows / Linux x64 / Linux arm64 |
| `release` | 打 tag 时打包 zip 并发布 Release | 仅 `v*` tag 触发 |

失败即阻断 `build`，所以 push 前本地可只做**格式对齐**（不算编译/测试）：

```powershell
# 只做格式化对齐，避免 CI 因格式挂掉；不编译、不跑测试
$env:PATH = "C:\Users\liuqi\.rustup\toolchains\stable-x86_64-pc-windows-msvc\bin;$env:PATH"
cd D:\demo\rust-api-proxy
cargo fmt --all
```

## 二、运行

二进制从 CI Artifacts / Release 下载后无需安装，直接运行。

一键启动（自动追加 PATH，可交互输入上游地址）：

```powershell
D:\demo\rust-api-proxy\start-relay.ps1
```

或手动：

```powershell
$env:PATH = "C:\Users\liuqi\.rustup\toolchains\stable-x86_64-pc-windows-msvc\bin;$env:PATH"
$env:UPSTREAM_URL = "https://xxx.trycloudflare.com/v1"
$env:MODEL_MAP    = "gpt-4=uncensored-27b,claude-sonnet-4=uncensored-27b"
$env:BIND_ADDR    = "127.0.0.1:8080"
.\uncensored-proxy.exe
```

端口选择 8080 的原因：cc-switch 本地代理固定占用 **15721**，8080 无冲突。

## 三、环境变量

| 变量 | 默认值 | 说明 |
|---|---|---|
| `UPSTREAM_URL` | `https://api.openai.com/v1` | 无审查模型服务地址 |
| `MODEL_MAP` | `gpt-4=uncensored-27b,claude-sonnet-4=uncensored-27b` | `客户端模型名=上游模型ID` 逗号分隔 |
| `BIND_ADDR` | `127.0.0.1:8080` | 监听地址，局域网访问填 `0.0.0.0:8080` |

## 四、接口

| 方法 | 路径 | 转发目标 | 改写模型名 |
|---|---|---|---|
| GET | `/health` | 本地响应，返回上游地址与映射表 | — |
| ANY | `/v1/chat/completions` | 上游 `/chat/completions` | 是 |
| ANY | `/v1/messages` | 上游 `/messages`（Anthropic 协议） | 是 |
| ANY | `/v1/models` | 上游 `/models` | 否 |

请求头剔除：`host`、`accept-encoding`、`content-length`
响应头剔除：`content-encoding`、`transfer-encoding`（reqwest 已解压，避免头声明与实不符）

## 五、接入本机 cc-switch

本机 cc-switch 已装（`C:\Users\liuqi\.cc-switch`），本地代理已在跑，端口 15721。

### 方式 A（推荐）：新增 Provider，Base URL 指向中转站

打开 cc-switch → 添加 Provider → 粘贴 `cc-switch-providers.json` 里对应配置。

**Claude（Anthropic 协议）** `settings_config`：
```json
{
  "env": {
    "ANTHROPIC_BASE_URL": "http://127.0.0.1:8080/v1",
    "ANTHROPIC_AUTH_TOKEN": "<上游无审查模型 Key>",
    "ANTHROPIC_MODEL": "uncensored-27b",
    "ANTHROPIC_DEFAULT_HAIKU_MODEL": "uncensored-27b",
    "ANTHROPIC_DEFAULT_SONNET_MODEL": "uncensored-27b",
    "ANTHROPIC_DEFAULT_OPUS_MODEL": "uncensored-27b",
    "ANTHROPIC_DEFAULT_FABLE_MODEL": "uncensored-27b",
    "CLAUDE_CODE_SUBAGENT_MODEL": "uncensored-27b"
  }
}
```

**Codex（OpenAI 协议）**：
```json
{
  "config": { "model_provider": "uncensored-relay", "model": "uncensored-27b" },
  "providers": {
    "uncensored-relay": {
      "name": "uncensored-relay",
      "base_url": "http://127.0.0.1:8080/v1",
      "env_key": "UNICENSORED_RELAY_KEY",
      "wire_api": "chat"
    }
  }
}
```

保存后把该 Provider 设为 current，Claude Code / Codex 的流量即经中转站打到无审查模型。

**不想手动建库**：也可以把 `ANTHROPIC_BASE_URL` 直接改成 `http://127.0.0.1:8080/v1`，
让 cc-switch 自己当一层，中转站当二层。两层代理无冲突，但多一跳延迟，不推荐。

## 六、接入 lain42.top

部署包已备好，见 `deploy/`：

| 文件 | 用途 |
|---|---|
| `deploy/uncensored-proxy.service` | systemd 单元，含安全加固 |
| `deploy/Caddyfile` | Caddy 反代（`flush_interval -1` 保持 SSE） |
| `deploy/nginx-lain42.conf` | Nginx 反代（`proxy_buffering off` 保持 SSE） |
| `deploy/deploy-lain42.sh` | 一键编译 Linux 二进制 + 上传 + 安装启动 |

关键点：反代必须关缓冲，否则 SSE 流会被攒包延迟。
Nginx 用 `proxy_buffering off`；Caddy 用 `flush_interval -1`。

## 七、验证

CI 是唯一验证入口。`cargo test` 覆盖 8 个单测：

| 测试 | 验证点 |
|---|---|
| `parse_model_map_handles_spaces_and_ignores_junk` | 空格容忍、正确解析 3 条 |
| `parse_model_map_ignores_malformed_entries` | 丢弃无 `=` 的脏数据 |
| `parse_model_map_empty_input` | 空串不崩 |
| `remap_known_models` | `gpt-4`/`claude-sonnet-4`/`claude-opus-4` → `uncensored-27b` |
| `remap_unknown_model_passes_through` | 未知模型名原样透传（不强行改写） |
| `remap_payload_without_model_is_untouched` | 无 model 字段的请求体不受影响 |
| `remap_preserves_other_fields` | 改写模型名时不动 messages/stream/temperature |
| `upstream_url_is_trimmed` | 上游地址尾斜杠被正确去除 |

已在本机做过的检查（仅格式与静态检查，未编译/未跑测试）：`cargo fmt -- --check` 无 diff，
`cargo clippy --all-targets -- -D warnings` 零告警 —— 与 CI 三项门槛一致。

## 八、文件清单

```
rust-api-proxy/
├── Cargo.toml                    # 依赖 + release 优化
├── Cargo.lock
├── src/main.rs                   # 中转站全部实现（约 290 行，含 8 个单测）
├── start-relay.ps1               # 一键启动（自动补 PATH）
├── cc-switch-providers.json      # cc-switch 两家 Provider 配置
├── .github/workflows/ci.yml  # CI：fmt + clippy + test + 3 平台构建
├── deploy/
│   ├── uncensored-proxy.service  # systemd 单元
│   ├── Caddyfile                 # Caddy 反代配置
│   ├── nginx-lain42.conf         # Nginx 反代配置
│   └── deploy-lain42.sh          # 一键部署脚本
```

## 九、接入的无审查上游

待填入 `UPSTREAM_URL`。按前面的调研，目标是 Kaggle TPU v5e-8 跑的
`llmfan46/Qwen3.8-27B-Uncensored-Heretic-Native-MTP-Preserved`（bf16，~130 tok/s），
通过 Cloudflare 隧道吐出的 `https://<tunnel>.trycloudflare.com/v1`。

上游启动后只需改一个环境变量重启中转站即可，代码零改动。
