# Downloads the signed PawnIO modules (namazso/PawnIO.Modules) used for CPU sensors.
# The PawnIO driver itself is installed separately: winget install namazso.PawnIO
$ErrorActionPreference = "Stop"

$targetDir = Join-Path $PSScriptRoot "..\src-tauri\resources\pawnio"
$modules = @("AMDFamily17.bin")

New-Item -ItemType Directory -Force -Path $targetDir | Out-Null

$release = Invoke-RestMethod -Uri "https://api.github.com/repos/namazso/PawnIO.Modules/releases/latest" `
    -Headers @{ "User-Agent" = "ChromaHUD" }
$asset = $release.assets | Where-Object { $_.name -like "*.zip" } | Select-Object -First 1
if (-not $asset) {
    throw "No zip asset found in PawnIO.Modules release $($release.tag_name)."
}

$zipPath = Join-Path $env:TEMP $asset.name
$extractDir = Join-Path $env:TEMP "pawnio-modules-$($release.tag_name)"
Write-Host "Downloading $($asset.name) from release $($release.tag_name)..."
Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $zipPath
Expand-Archive -Path $zipPath -DestinationPath $extractDir -Force

foreach ($module in $modules) {
    $file = Get-ChildItem -Path $extractDir -Recurse -Filter $module | Select-Object -First 1
    if (-not $file) {
        throw "Module $module not found in the release archive."
    }
    Copy-Item $file.FullName (Join-Path $targetDir $module) -Force
    Write-Host "Saved $module"
}
