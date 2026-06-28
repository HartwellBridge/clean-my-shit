# Build the Windows release binary and the NSIS installer, optionally
# Authenticode-signed.
#
# Requires: Rust toolchain and NSIS (makensis) on PATH.
#   winget install NSIS.NSIS   (or: choco install nsis)
#
# Optional code signing (needs signtool from the Windows SDK on PATH).
# Set these env vars to sign the .exe and the installer:
#   WINDOWS_CERT_PFX        path to a code-signing certificate (.pfx)
#   WINDOWS_CERT_PASSWORD   its password
#   WINDOWS_TIMESTAMP_URL   optional, defaults to DigiCert's RFC-3161 server
#
# Usage:  powershell -ExecutionPolicy Bypass -File packaging\windows\build.ps1
# Output: dist\CleanMyShit-Setup.exe

$ErrorActionPreference = "Stop"
$root = (Resolve-Path "$PSScriptRoot\..\..").Path

$certPfx = $env:WINDOWS_CERT_PFX
$certPwd = $env:WINDOWS_CERT_PASSWORD
$tsUrl = if ($env:WINDOWS_TIMESTAMP_URL) { $env:WINDOWS_TIMESTAMP_URL } else { "http://timestamp.digicert.com" }

function Invoke-Sign($path) {
    if (-not $certPfx) {
        return  # no certificate configured -> unsigned build
    }
    Write-Host "==> Signing $path"
    & signtool sign /fd SHA256 /tr $tsUrl /td SHA256 /f $certPfx /p $certPwd $path
    if ($LASTEXITCODE -ne 0) { throw "signtool failed for $path" }
}

Write-Host "==> Building release binary"
cargo build --release --manifest-path "$root\Cargo.toml"

Invoke-Sign "$root\target\release\clean-my-shit.exe"

if (-not (Test-Path "$root\assets\icon.ico")) {
    Write-Host "==> Generating icon assets"
    cargo run --release --manifest-path "$root\tools\iconforge\Cargo.toml" -- "$root\assets"
}

New-Item -ItemType Directory -Force -Path "$root\dist" | Out-Null

Write-Host "==> Building installer with NSIS"
& makensis "$root\packaging\windows\installer.nsi"

Invoke-Sign "$root\dist\CleanMyShit-Setup.exe"

Write-Host "==> Done: $root\dist\CleanMyShit-Setup.exe"
