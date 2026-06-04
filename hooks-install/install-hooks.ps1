$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$SrcScripts = Join-Path $ScriptDir "scripts"
$DestDir = Join-Path $env:USERPROFILE ".cursor\hooks\agent-traffic-light"
$HooksJson = Join-Path $env:USERPROFILE ".cursor\hooks.json"
$Example = Join-Path $ScriptDir "hooks.user.json.example"

New-Item -ItemType Directory -Force -Path $DestDir | Out-Null
Copy-Item -Path (Join-Path $SrcScripts "*") -Destination $DestDir -Force

Write-Host "已复制脚本到: $DestDir"
Write-Host ""
Write-Host "请将 hooks.user.json.example 中的 hooks 段合并进:"
Write-Host "  $HooksJson"
Write-Host ""
Write-Host "示例文件:"
Write-Host "  $Example"
Write-Host ""
Write-Host "合并后重启 Cursor，并运行 npm run tauri dev 启动桌面灯。"
