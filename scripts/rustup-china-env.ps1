# 为用户级环境变量配置 rustup 清华镜像，执行后请重启终端
[System.Environment]::SetEnvironmentVariable(
    "RUSTUP_DIST_SERVER",
    "https://mirrors.tuna.tsinghua.edu.cn/rustup",
    "User"
)
[System.Environment]::SetEnvironmentVariable(
    "RUSTUP_UPDATE_ROOT",
    "https://mirrors.tuna.tsinghua.edu.cn/rustup/rustup",
    "User"
)
Write-Host "已设置 RUSTUP_DIST_SERVER / RUSTUP_UPDATE_ROOT（用户级）"
Write-Host "请关闭本窗口，新开 PowerShell 或 Git Bash，执行:"
Write-Host "  rustup default stable"
Write-Host "  cargo --version"
