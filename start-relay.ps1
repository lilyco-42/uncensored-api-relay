# 一键启动 Rust 无审查 API 中转站
$ErrorActionPreference = 'Stop'
$toolchain = 'C:\Users\liuqi\.rustup\toolchains\stable-x86_64-pc-windows-msvc\bin'
$env:PATH = "$toolchain;$env:PATH"

if (!$env:UPSTREAM_URL) {
    Write-Host '[提示] 未设置 UPSTREAM_URL，请填无审查模型服务地址：' -ForegroundColor Yellow
    $env:UPSTREAM_URL = Read-Host 'UPSTREAM_URL'
}
if (!$env:MODEL_MAP) {
    $env:MODEL_MAP = 'gpt-4=uncensored-27b,claude-sonnet-4=uncensored-27b'
}
if (!$env:BIND_ADDR) {
    $env:BIND_ADDR = '127.0.0.1:8080'
}

Write-Host ''
Write-Host "上游   : $env:UPSTREAM_URL" -ForegroundColor Cyan
Write-Host "映射   : $env:MODEL_MAP" -ForegroundColor Cyan
Write-Host "监听   : http://$env:BIND_ADDR" -ForegroundColor Cyan
Write-Host 'cc-switch Base URL: ' -NoNewline -ForegroundColor Cyan
Write-Host "http://$env:BIND_ADDR/v1"
Write-Host ''

& '.\target\release\uncensored-proxy.exe'
