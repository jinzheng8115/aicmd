Write-Error @'
AICmd does not support native Windows.

Please use one of these supported environments:
- macOS
- Linux
- Windows WSL with the Linux installer

WSL install command:
  curl -fsSL https://raw.githubusercontent.com/jinzheng8115/aicmd/main/contrib/aicmd/install.sh | bash

AICmd 不再支持 Windows 原生 PowerShell/cmd 环境。
请使用 macOS、Linux，或在 Windows WSL 中使用 Linux 安装方式。
'@
exit 1
