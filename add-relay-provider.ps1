# add-relay-provider.ps1  -  把 uncensored 中转站注册进 cc-switch
# 用法: .\add-relay-provider.ps1 -Upstream https://xxx.trycloudflare.com/v1
param(
    [Parameter(Mandatory=$true)][string]$Upstream,
    [string]$ApiKey = "sk-unc27b",
    [string]$Relay = "http://127.0.0.1:8080",
    [string]$DbPath = (Join-Path $env:USERPROFILE ".cc-switch\cc-switch.db")
)
if (-not (Test-Path $DbPath)) { throw ("cc-switch DB not found: " + $DbPath) }
$tplPath = Join-Path $PSScriptRoot "relay-provider.sql"
$tpl = Get-Content -Raw $tplPath
$sql = $tpl.Replace("__RELAY__", $Relay).Replace("__KEY__", $ApiKey)
$sql = $sql.Replace("__CID__", [guid]::NewGuid().ToString()).Replace("__XID__", [guid]::NewGuid().ToString())
$sql = $sql.Replace("__NOW__", [string][DateTimeOffset]::UtcNow.ToUnixTimeSeconds())
$bak = $DbPath + ".bak-" + (Get-Date -Format yyyyMMdd-HHmmss)
Copy-Item $DbPath $bak -Force
Write-Host ("backup: " + $bak)
$sqlFile = Join-Path $env:TEMP "relay-provider-run.sql"
Set-Content -Path $sqlFile -Value $sql -Encoding UTF8
& "D:\APP\scoop\shims\sqlite3.exe" $DbPath (".read " + $sqlFile)
if ($LASTEXITCODE -ne 0) { throw "sqlite insert failed" }
Write-Host ""
Write-Host ("relay    : " + $Relay + "/v1")
Write-Host ("upstream : " + $Upstream)
Write-Host "在 cc-switch 界面点一下 Uncensored Relay 启用；需重启 cc-switch 才重读 DB"
