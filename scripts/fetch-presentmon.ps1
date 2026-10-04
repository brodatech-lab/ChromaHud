# Downloads the latest PresentMon console application (MIT, GameTechDev/PresentMon)
# and places it where the Tauri sidecar mechanism expects it.
$ErrorActionPreference = "Stop"

$targetDir = Join-Path $PSScriptRoot "..\src-tauri\binaries"
$targetFile = Join-Path $targetDir "presentmon-x86_64-pc-windows-msvc.exe"

New-Item -ItemType Directory -Force -Path $targetDir | Out-Null

$release = Invoke-RestMethod -Uri "https://api.github.com/repos/GameTechDev/PresentMon/releases/latest" `
    -Headers @{ "User-Agent" = "ChromaHUD" }

$asset = $release.assets |
    Where-Object { $_.name -match '^PresentMon-[\d\.]+-x64\.exe$' } |
    Select-Object -First 1

if (-not $asset) {
    throw "Could not find a PresentMon x64 console executable in release $($release.tag_name)."
}

Write-Host "Downloading $($asset.name) from release $($release.tag_name)..."
Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $targetFile
Write-Host "Saved to $targetFile"
