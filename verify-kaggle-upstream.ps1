# verify-kaggle-upstream.ps1
# 端到端验证：Kaggle 无审查端点 -> Rust 中转站 -> OpenAI/Anthropic 两套协议
#
# 用法：
#   powershell -ExecutionPolicy Bypass -File .\verify-kaggle-upstream.ps1
# 或用变量覆盖：
#   $env:UPSTREAM_URL = "https://xxx.trycloudflare.com/v1"; .\verify-kaggle-upstream.ps1

param(
    [string]$RelayExe = ".\uncensored-proxy.exe"
)

$ErrorActionPreference = "Stop"
if (-not $env:UPSTREAM_URL) {
    Write-Host "!! UPSTREAM_URL 未设置。先编辑 env-presets.ps1 或直接 `$env:UPSTREAM_URL = "https://xxx.trycloudflare.com/v1"" -ForegroundColor Red
    exit 1
}

Write-Host "上游 : $env:UPSTREAM_URL"

# 1) 直接打上游
Write-Host "`n[1/5] 上游 /v1/models ..."
try {
    $m = Invoke-RestMethod -Uri "$env:UPSTREAM_URL/models" -TimeoutSec 30
    Write-Host "      OK, models: " (($m.data | ForEach-Object { $_.id }) -join ", ")
} catch {
    Write-Host "      FAIL: $_" -ForegroundColor Red; exit 1
}

# 2) 起中转站
Write-Host "`n[2/5] 启动中转站 ..."
$env:BIND_ADDR = "127.0.0.1:8080"
if (-not $env:MODEL_MAP) {
    $env:MODEL_MAP = "gpt-4=uncensored-27b,claude-sonnet-4=uncensored-27b"
}
$proc = Start-Process -FilePath $RelayExe -PassThru -WindowStyle Hidden
Start-Sleep -Seconds 2
try {
    $h = Invoke-RestMethod -Uri "http://127.0.0.1:8080/health" -TimeoutSec 10
    Write-Host "      health OK, upstream = $($h.upstream)"
} catch {
    Write-Host "      health FAIL: $_" -ForegroundColor Red; Stop-Process -Id $proc.Id -Force; exit 1
}

function Ask($path, $bodyObj) {
    $j = $bodyObj | ConvertTo-Json -Depth 6 -Compress
    Invoke-RestMethod -Uri "http://127.0.0.1:8080$path" -Method Post `
        -ContentType "application/json" -Body $j -Headers @{ "Authorization" = "Bearer $env:UNCENSORED_API_KEY" }
}

# 3) OpenAI 协议 + model remap
Write-Host "`n[3/5] OpenAI 协议 + 模型映射 ..."
try {
    $r = Ask "/v1/chat/completions" @{ model = "gpt-4"; messages = @(@{role="user"; content="用一句话说你好"}); max_tokens = 48 }
    $t = $r.choices[0].message.content
    $refused = ($t -match "我不能|我无法|抱歉|作为 ?AI|不提供|不予")
    Write-Host "      模型: $($r.model)  内容: $($t.Substring(0,[Math]::Min(60,$t.Length)))"
    Write-Host "      拒答: $refused"
} catch {
    Write-Host "      FAIL: $_" -ForegroundColor Red
}

# 4) Anthropic 协议（cc-switch / Claude Code 走这条）
Write-Host "`n[4/5] Anthropic 协议 + 模型映射 ..."
try {
    $a = Ask "/v1/messages" @{ model = "claude-sonnet-4-20250514"; max_tokens = 48; messages = @(@{role="user"; content="用一句话说你好"}) }
    $t = $a.content[0].text
    $refused = ($t -match "我不能|我无法|抱歉|作为 ?AI|不提供|不予")
    Write-Host "      模型: $($a.model)  内容: $($t.Substring(0,[Math]::Min(60,$t.Length)))"
    Write-Host "      拒答: $refused"
} catch {
    Write-Host "      FAIL: $_" -ForegroundColor Red
}

# 5) 流式
Write-Host "`n[5/5] SSE 流式 ..."
try {
    $j = @{ model = "gpt-4"; messages = @(@{role="user"; content="数到 5"}); max_tokens = 32; stream = $true } | ConvertTo-Json -Depth 6 -Compress
    $resp = Invoke-WebRequest -Uri "http://127.0.0.1:8080/v1/chat/completions" -Method Post `
        -ContentType "application/json" -Body $j -Headers @{ "Authorization" = "Bearer $env:UNCENSORED_API_KEY" } -TimeoutSec 60
    $chunks = ($resp.Content -split "data: " | Where-Object { $_ -match "DONE|content" }).Count
    Write-Host "      SSE 原始长度: $($resp.Content.Length), 看起来有 $chunks 个 data 块"
} catch {
    Write-Host "      FAIL: $_" -ForegroundColor Red
}

Stop-Process -Id $proc.Id -Force
Write-Host "`n完成。中转站已停止。"
