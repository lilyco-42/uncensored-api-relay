# ============================================================
#  uncensored-proxy 环境变量预设
#  用法（PowerShell）：
#    . .\env-presets.ps1
#    .\uncensored-proxy.exe
# ============================================================

# --- 预设 1：上游 = 本机 Kaggle TPU 隧道（无审查 27B 主路径）---
# $env:UPSTREAM_URL = "https://<tunnel-id>.trycloudflare.com/v1"

# --- 预设 2：上游 = lain42.top:3000 NewAPI 面板 ----------------
# 注意：实测该端口当前返回 502，面板后端未启动。
# 面板恢复后用这套配置即可让 cc-switch 走 Rust 中转站。
$env:UPSTREAM_URL = "http://lain42.top:3000/v1"

# --- 预设 3：上游 = 公网 OpenAI 兼容服务 ----------------------
# $env:UPSTREAM_URL = "https://api.openai.com/v1"

# 模型映射：客户端模型名 -> 上游无审查模型 ID
$env:MODEL_MAP = "gpt-4=uncensored-27b,claude-sonnet-4=uncensored-27b,claude-opus-4=uncensored-27b,grok-4=uncensored-27b"

# 监听地址。0.0.0.0 表示允许局域网/容器访问；仅本机用 127.0.0.1
$env:BIND_ADDR = "127.0.0.1:8080"

Write-Host ""
Write-Host "UPSTREAM_URL : $env:UPSTREAM_URL"
Write-Host "MODEL_MAP    : $env:MODEL_MAP"
Write-Host "BIND_ADDR    : $env:BIND_ADDR"
Write-Host "cc-switch    : ANTHROPIC_BASE_URL=http://$env:BIND_ADDR"
Write-Host ""
