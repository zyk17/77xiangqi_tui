# 本地质量门禁：测试 + 严格 Clippy（warnings 即失败）
$ErrorActionPreference = "Stop"
Set-Location (Split-Path $PSScriptRoot -Parent)

cargo test
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

cargo lint-strict
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "check.ps1: OK"
