param(
  [switch]$FromSource,
  [string]$Version = $env:AICMD_VERSION,
  [string]$Repo = $(if ($env:AICMD_REPO) { $env:AICMD_REPO } else { "jinzheng8115/aicmd" }),
  [string]$BinDir = $env:AICMD_INSTALL_BIN_DIR,
  [switch]$NoProfile
)

$ErrorActionPreference = "Stop"

function Get-UserHome() {
  if ($HOME) { return $HOME }
  if ($env:USERPROFILE) { return $env:USERPROFILE }
  $profileDir = [Environment]::GetFolderPath("UserProfile")
  if ($profileDir) { return $profileDir }
  throw "Unable to detect the current user's home directory"
}

function Write-DefaultMcpConfig($Path) {
  $content = @'
{
  "mcp": {
    "servers": {
      "tavily": {
        "type": "stdio",
        "command": "npx",
        "args": ["-y", "tavily-mcp"],
        "env": {
          "TAVILY_API_KEY": "tvly-xxxx"
        }
      },
      "context7": {
        "type": "stdio",
        "command": "npx",
        "args": ["-y", "@upstash/context7-mcp"]
      }
    },
    "commands": {
      "search": {
        "description": "Search the web using Tavily",
        "server": "tavily"
      },
      "context7-library": {
        "description": "Resolve a package/library name using Context7",
        "server": "context7"
      }
    }
  }
}
'@
  Set-Content -Path $Path -Value $content -Encoding UTF8
}

function New-Wrapper($Name, $ArgsLine) {
  $path = Join-Path $BinDir "$Name.cmd"
  $content = "@echo off`r`n`"%~dp0aicmd.exe`" $ArgsLine %*`r`n"
  Set-Content -Path $path -Value $content -Encoding ASCII
  return $path
}

function Get-Target() {
  $archValue = $env:PROCESSOR_ARCHITEW6432
  if (-not $archValue) { $archValue = $env:PROCESSOR_ARCHITECTURE }
  if (-not $archValue) { throw "Unable to detect Windows architecture" }
  $arch = "$archValue".ToLowerInvariant()
  switch ($arch) {
    "amd64" { return "x86_64-pc-windows-msvc" }
    "x64" { return "x86_64-pc-windows-msvc" }
    "x86_64" { return "x86_64-pc-windows-msvc" }
    "arm64" { return "aarch64-pc-windows-msvc" }
    default { throw "Unsupported Windows architecture: $arch" }
  }
}

function Get-LatestVersion() {
  if ($Version) { return $Version }
  $release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest" -Headers @{"User-Agent"="aicmd-installer"}
  return $release.tag_name
}

function Invoke-AICmdInstall() {
if (-not $BinDir) {
  $BinDir = Join-Path (Get-UserHome) ".local\bin"
}

New-Item -ItemType Directory -Force -Path $BinDir | Out-Null

if ($FromSource) {
  if (-not $PSScriptRoot) { throw "-FromSource requires running install.ps1 from a checked-out repository" }
  $rootDir = Resolve-Path (Join-Path $PSScriptRoot "..\..")
  $cargo = if ($env:CARGO) { $env:CARGO } else { "cargo" }
  & $cargo build --release --manifest-path (Join-Path $rootDir "Cargo.toml")
  if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
  Copy-Item -Force (Join-Path $rootDir "target\release\aicmd.exe") (Join-Path $BinDir "aicmd.exe")
} else {
  $target = Get-Target
  $tag = Get-LatestVersion
  $archive = "aicmd-$tag-$target.zip"
  $url = "https://github.com/$Repo/releases/download/$tag/$archive"
  $tmp = Join-Path ([System.IO.Path]::GetTempPath()) "aicmd-install-$([System.Guid]::NewGuid())"
  New-Item -ItemType Directory -Force -Path $tmp | Out-Null
  try {
    $zipPath = Join-Path $tmp $archive
    Invoke-WebRequest -Uri $url -OutFile $zipPath -Headers @{"User-Agent"="aicmd-installer"}
    Expand-Archive -Force -Path $zipPath -DestinationPath $tmp
    Copy-Item -Force (Join-Path $tmp "aicmd.exe") (Join-Path $BinDir "aicmd.exe")
  } finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
  }
}

$wrappers = @()
$wrappers += New-Wrapper "aicmd-model" "model"
$wrappers += New-Wrapper "aicmd-mcp" "mcp-raw"
$wrappers += New-Wrapper "aicmd-err" "err"
$wrappers += New-Wrapper "aicmd-do" "do"
$wrappers += New-Wrapper "aicmd-shell-init" "shell-init"

$configDir = if ($env:AICMD_CONFIG_DIR) { $env:AICMD_CONFIG_DIR } else { Join-Path (Get-UserHome) ".aicmd" }
New-Item -ItemType Directory -Force -Path $configDir | Out-Null
$mcpPath = if ($env:AICMD_MCP_CONFIG_FILE) { $env:AICMD_MCP_CONFIG_FILE } else { Join-Path $configDir "mcp.json" }
if (-not (Test-Path $mcpPath)) {
  Write-DefaultMcpConfig $mcpPath
  $mcpStatus = "Installed MCP config: $mcpPath"
} else {
  $mcpStatus = "Existing MCP config kept: $mcpPath"
}
$configPath = Join-Path $configDir "config.yaml"
if (Test-Path $configPath) {
  $configStatus = "Existing config kept: $configPath"
} else {
  $configStatus = "No config found. Copy .env.example to .env, fill it, then run: aicmd init --from-env"
}

$currentUserPath = [Environment]::GetEnvironmentVariable("Path", "User")
if (-not (($currentUserPath -split ';') -contains $BinDir)) {
  $newUserPath = if ([string]::IsNullOrWhiteSpace($currentUserPath)) { $BinDir } else { "$currentUserPath;$BinDir" }
  [Environment]::SetEnvironmentVariable("Path", $newUserPath, "User")
  $env:Path = "$env:Path;$BinDir"
  $pathStatus = "Added to user PATH: $BinDir"
} else {
  $pathStatus = "Already in user PATH: $BinDir"
}

$profileLine = 'Invoke-Expression (& aicmd shell-init powershell)'
if (-not $NoProfile) {
  $profilePath = "$PROFILE"
  if (-not $profilePath) {
    $profilePath = Join-Path (Get-UserHome) "Documents\WindowsPowerShell\Microsoft.PowerShell_profile.ps1"
  }
  $profileDir = Split-Path -Parent $profilePath
  if ($profileDir) { New-Item -ItemType Directory -Force -Path $profileDir | Out-Null }
  if (-not (Test-Path $profilePath)) { New-Item -ItemType File -Force -Path $profilePath | Out-Null }
  $profileText = Get-Content -Path $profilePath -ErrorAction SilentlyContinue | Out-String
  if (-not $profileText) { $profileText = "" }
  if ($profileText -notmatch [regex]::Escape($profileLine)) {
    Add-Content -Path $profilePath -Value "`n# >>> aicmd shell integration >>>`n$profileLine`n# <<< aicmd shell integration <<<"
    $profileStatus = "Installed into PowerShell profile: $profilePath"
  } else {
    $profileStatus = "PowerShell profile already contains AICmd integration: $profilePath"
  }
} else {
  $profileStatus = "Skipped PowerShell profile integration. Add manually: $profileLine"
}

Write-Host "Installed AICmd to: $(Join-Path $BinDir 'aicmd.exe')"
Write-Host "Installed compatibility wrappers:"
$wrappers | ForEach-Object { Write-Host "  $_" }
Write-Host "PATH:"
Write-Host "  $pathStatus"
Write-Host "Shell integration:"
Write-Host "  $profileStatus"
Write-Host "  Restart PowerShell or run: $profileLine"
Write-Host "Config:"
Write-Host "  $configStatus"
Write-Host "  $mcpStatus"
Write-Host "Make sure $BinDir is in PATH, then run:"
Write-Host "  aicmd 列出当前目录最大的 10 个文件"
}

try {
  Invoke-AICmdInstall
} catch {
  [Console]::Error.WriteLine("AICmd install failed: $($_.Exception.Message)")
  if ($_.InvocationInfo) {
    [Console]::Error.WriteLine("At $($_.InvocationInfo.ScriptName):$($_.InvocationInfo.ScriptLineNumber)")
    [Console]::Error.WriteLine("$($_.InvocationInfo.Line)")
  }
  throw
}
