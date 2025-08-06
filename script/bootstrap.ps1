#!/usr/bin/env pwsh
$RepoRoot = Split-Path $PSScriptRoot -Parent

# Check for dependencies we don't automatically install
& "$RepoRoot/script/check-deps.ps1"

Write-Host "Installing Python build tools..."
pip install maturin
pip install poetry
poetry config virtualenvs.in-project true

Write-Host "Installing Python dependencies..."
poetry -C "./python" install

$hostTarget = ((rustc -vV | Select-String "host: ") -split ':')[1].Trim()
Write-Host "Installing Rust dependencies using host target '$hostTarget' ..."
cargo install --target "$hostTarget" --locked cbindgen@0.29.0
cargo install --target "$hostTarget" --locked just@1.42.4

Write-Host "Installing addlicense..."
go install github.com/google/addlicense@v1.1.1
Write-Host "Go env"
go env
Write-Host "Addlicense path"
Get-Command addlicense -ErrorAction Continue

if (-not [string]::IsNullOrEmpty($env:BUILD_BUILDID)) {
    # We're in a CI environment, so put the GOBIN on the system path
    Write-Host "##vso[task.prependpath]$env:GOPATH/bin"
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
