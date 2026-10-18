# ============================================================
#  uncensored-proxy 环境变量预设
#
#  用法（PowerShell）：
#    . .\env-presets.ps1        # 加载预设（Kaggle 无审查 27B）
#    .\uncensored-proxy.exe
#
#  切换上游：改下面被啟用的那一行，或命令行临时覆盖：
#    $env:UPSTREAM_URL = "http://lain42.top:3000/v1"
# ============================================================
# --- 预设 1（默认）：Kaggle 无审查 27B ----------------------
# 模型 llmfan46/Qwen3.8-27B-Uncensored-Heretic-Native-MTP-Preserved-GPTQ-Int4
# 由 uncensored-relay-kernel 在 Kaggle 2xTesla T4 上用 vLLM gptq_marlin 跑，
# cloudflared quick tunnel 暴露。地址每次重启 notebook 都会变，
# 以 /kaggle/working/endpoint.txt 为准（见 kernel 输出）。
# 把下面这行的 <tunnel-id> 换成真实地址，并取消注释：
# $env:UPSTREAM_URL = "https://<tunnel-id>.trycloudflare.com/v1"

# --- 预设 2：lain42.top:3000 NewAPI 面板 ---------------------
# 注意：实测该端口当前返回 502，面板后端未启动。
# $env:UPSTREAM_URL = "http://lain42.top:3000/v1"

# --- 预设 3：公网 OpenAI 兼容服务（备用/自备 key）-------------
# $env:UPSTREAM_URL = "https://api.openai.com/v1"

# 命令行已经设置过 UPSTREAM_URL 就沿用，否则留空由下面的检查拦住
if (-not $env:UPSTREAM_URL) { $env:UPSTREAM_URL = "" }

# 模型映射：客户端模型名 -> 上游无审查模型 ID。
# 逗号分隔；左侧是 cc-switch/客户端发来的名字，右侧是上游 served-model-name。
$env:MODEL_MAP = "gpt-4=uncensored-27b,gpt-4o=uncensored-27b,gpt-3.5-turbo=uncensored-27b,claude-sonnet-4=uncensored-27b,claude-3-5-sonnet-20241022=uncensored-27b,claude-3-7-sonnet-20250219=uncensored-27b,claude-opus-4-20250514=uncensored-27b,grok-4=uncensored-27b"

# 监听地址。0.0.0.0 表示允许局域网/容器访问；仅本机用 127.0.0.1
# cc-switch 走本机代理，所以默认 127.0.0.1 就够。
$env:BIND_ADDR = "127.0.0.1:8080"

if (-not $env:UPSTREAM_URL) {
    Write-Host ""
    Write-Host "[!] UPSTREAM_URL 为空。请先编辑本文件，取消某个预设的注释，再执行 . .\env-presets.ps1" -ForegroundColor Yellow
    Write-Host ""
    return
}

Write-Host ""
Write-Host "UPSTREAM_URL : $env:UPSTREAM_URL"
Write-Host "MODEL_MAP    : $env:MODEL_MAP"
Write-Host "BIND_ADDR    : $env:BIND_ADDR"
Write-Host "cc-switch    : ANTHROPIC_BASE_URL=http://$env:BIND_ADDR"
Write-Host ""
