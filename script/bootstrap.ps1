#!/usr/bin/env pwsh
$RepoRoot = Split-Path $PSScriptRoot -Parent

# Checks if the command exists in the system path
function Test-Command {
    param (
        [string]$Command,
        [switch]$Require
    )
    $commandPath = Get-Command $Command -ErrorAction SilentlyContinue
    $found = $commandPath -ne $null
    if ($Require -and -not $found) {
        throw "Command '$Command' is required but not found in the system path."
    }
}

# Check for dependencies we don't automatically install
Test-Command cargo -Require
Test-Command go -Require

if (!(Test-Command just)) {
    Write-Host "Installing just..."
    cargo install just
}

if (Test-Command python) {
    Write-Host "Installing Python build tools..."
    if (-not (Test-Command "maturin")) {
        pip install maturin
    }
    if (-not (Test-Command "poetry")) {
        pip install poetry
    }
    poetry config virtualenvs.in-project true

    Write-Host "Installing Python dependencies..."
    poetry -C "./python" install
}

$hostTarget = ((rustc -vV | Select-String "host: ") -split ':')[1].Trim()
Write-Host "Installing Rust dependencies using host target '$hostTarget' ..."
if (-not (Test-Command "cbindgen")) {
    cargo install --target "$hostTarget" --locked cbindgen@0.29.0 --config .cargo/config.toml
}
if (-not (Test-Command "just")) {
    cargo install --target "$hostTarget" --locked just@1.42.4 --config .cargo/config.toml
}

Write-Host "Installing addlicense..."
if (-not (Test-Command "addlicense")) {
    go install github.com/google/addlicense@v1.1.1
}

if (-not [string]::IsNullOrEmpty($env:BUILD_BUILDID)) {
    # We're in a CI environment, so put the GOBIN on the system path
    $gopath = (go env GOPATH)
    Write-Host "##vso[task.prependpath]$gopath/bin"
}

if ($IsWindows) {
    # Check for msys and install the msys runtime if available
    if (Test-Path "C:\mingw64\usr\bin\pacman.exe") {
        Write-Host "Installing MSYS2 runtime for GNU builds..."
        & "C:\mingw64\usr\bin\pacman.exe" -Syu --noconfirm
        & "C:\mingw64\usr\bin\pacman.exe" -S --noconfirm msys2-w32api-runtime
    }
    else {
        Write-Host "Unable to find MSYS Pacman executable. Skipping MSYS2 runtime installation."
    }
}
