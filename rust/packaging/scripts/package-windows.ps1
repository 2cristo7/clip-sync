# Creates zip archives for ClipSync Windows binaries.
#
# Usage: .\package-windows.ps1 [-Version "0.2.0"]
# Outputs: dist\clipsync-server-<version>-windows-x86_64.zip
#          dist\clipsync-client-<version>-windows-x86_64.zip

param(
    [string]$Version = "0.1.0"
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RustDir = Resolve-Path "$ScriptDir\..\.."
$DistDir = "$RustDir\dist"

Write-Host "==> Packaging Windows zip archives (version $Version)"

# Build release binaries
Write-Host "==> Building release binaries..."
Set-Location $RustDir
cargo build --release -p clipsync-server -p clipsync-client

if (-not (Test-Path $DistDir)) {
    New-Item -ItemType Directory -Path $DistDir | Out-Null
}

foreach ($Binary in @("clipsync-server", "clipsync-client")) {
    $BinaryPath = "$RustDir\target\release\$Binary.exe"
    if (-not (Test-Path $BinaryPath)) {
        Write-Error "Binary not found at $BinaryPath"
        exit 1
    }

    $ZipName = "$Binary-$Version-windows-x86_64.zip"
    $ZipPath = "$DistDir\$ZipName"
    $StagingDir = "$DistDir\$Binary-staging"

    Write-Host "==> Creating $ZipName..."

    # Clean staging area
    if (Test-Path $StagingDir) {
        Remove-Item -Recurse -Force $StagingDir
    }
    New-Item -ItemType Directory -Path $StagingDir | Out-Null

    # Copy binary
    Copy-Item $BinaryPath "$StagingDir\$Binary.exe"

    # Create zip
    if (Test-Path $ZipPath) {
        Remove-Item -Force $ZipPath
    }
    Compress-Archive -Path "$StagingDir\*" -DestinationPath $ZipPath

    # Clean up staging
    Remove-Item -Recurse -Force $StagingDir

    Write-Host "    Created: $ZipPath"
}

Write-Host "==> Done. Archives in $DistDir\"
Get-ChildItem "$DistDir\*.zip" | Format-Table Name, Length
