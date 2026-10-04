# Starts `npm run tauri dev` in an elevated PowerShell window (UAC prompt) so PresentMon and WMI work.
# Output is mirrored to %TEMP%\chromahud-dev.log for troubleshooting.
param([switch]$Elevated)

$projectRoot = Split-Path $PSScriptRoot -Parent
$logFile = Join-Path $env:TEMP "chromahud-dev.log"

if (-not $Elevated) {
    Start-Process powershell -Verb RunAs -ArgumentList @(
        "-NoExit", "-ExecutionPolicy", "Bypass",
        "-File", "`"$PSCommandPath`"", "-Elevated"
    )
    return
}

$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
Set-Location $projectRoot
Start-Transcript -Path $logFile -Force | Out-Null
try {
    npm run tauri dev 2>&1 | ForEach-Object { "$_" }
}
finally {
    Stop-Transcript | Out-Null
}
