# ChromaHUD one-line installer.
#   irm https://github.com/brodatech-lab/ChromaHud/releases/latest/download/install.ps1 | iex
# With the optional PawnIO driver (AMD CPU temperature / power / per-core clocks):
#   & ([scriptblock]::Create((irm https://github.com/brodatech-lab/ChromaHud/releases/latest/download/install.ps1))) -WithPawnIO
param(
    [switch]$WithPawnIO,
    [switch]$NoLaunch
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

$repo = "brodatech-lab/ChromaHud"

Write-Host "ChromaHUD installer" -ForegroundColor Cyan

$release = Invoke-RestMethod -Uri "https://api.github.com/repos/$repo/releases/latest" -Headers @{ "User-Agent" = "ChromaHUD-installer" }
$asset = $release.assets | Where-Object { $_.name -like "*_x64-setup.exe" } | Select-Object -First 1
if (-not $asset) {
    throw "No installer found in release $($release.tag_name)."
}

$setup = Join-Path $env:TEMP $asset.name
Write-Host "Downloading $($asset.name) ($($release.tag_name))..."
Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $setup

Write-Host "Installing to Program Files (confirm the administrator prompt)..."
$process = Start-Process -FilePath $setup -ArgumentList "/S" -Wait -PassThru
Remove-Item $setup -ErrorAction SilentlyContinue
if ($process.ExitCode -ne 0) {
    throw "Installer exited with code $($process.ExitCode). Close ChromaHUD if it is running and try again."
}
Write-Host "ChromaHUD $($release.tag_name) installed." -ForegroundColor Green

if ($WithPawnIO) {
    if (Get-Command winget -ErrorAction SilentlyContinue) {
        Write-Host "Installing the PawnIO driver (administrator prompt)..."
        winget install namazso.PawnIO --source winget --accept-package-agreements --accept-source-agreements
    } else {
        Write-Warning "winget is not available. Install PawnIO manually from https://pawnio.eu/"
    }
}

$exe = Join-Path $env:ProgramFiles "ChromaHUD\chromahud.exe"
if (-not $NoLaunch -and (Test-Path $exe)) {
    Write-Host "Starting ChromaHUD (it asks for administrator rights to read FPS and CPU sensors)..."
    Start-Process -FilePath $exe
}

Write-Host ""
Write-Host "Ctrl+Shift+H  show / hide the overlay"
Write-Host "Ctrl+Shift+O  settings (or click the tray icon)"
Write-Host "Ctrl+Shift+C  per-core CPU details"
