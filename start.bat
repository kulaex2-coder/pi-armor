<# : batch
@echo off
>nul chcp 65001
title Pi Armor
set "CARGO_TARGET_DIR=C:\rust-targets\pi-armor"
cd /d "%~dp0"
powershell -NoProfile -ExecutionPolicy Bypass -Command "iex (Get-Content -LiteralPath '%~f0' -Raw -Encoding UTF8)"
if errorlevel 1 (
  echo.
  echo Failed - see messages above.
  pause
  exit /b 1
)
exit /b 0
#>
# ===== PowerShell 区 =====
$ErrorActionPreference = 'Stop'
try {
    Write-Host 'Pi Armor dev 模式启动中，首次编译需要几分钟...' -ForegroundColor Cyan
    if (-not (Test-Path -LiteralPath 'node_modules\@tauri-apps\cli')) {
        Write-Host '缺少依赖，先执行 npm install...' -ForegroundColor Yellow
        & npm.cmd install
        if ($LASTEXITCODE -ne 0) { throw "npm install 失败，退出码 $LASTEXITCODE" }
    }
    & npm.cmd run tauri dev
    if ($LASTEXITCODE -ne 0) { throw "tauri dev 退出码 $LASTEXITCODE" }
    exit 0
} catch {
    Write-Host "出错：$_" -ForegroundColor Red
    exit 1
}