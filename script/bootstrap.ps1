$RepoRoot = Split-Path $PSScriptRoot -Parent

if (-not $IsWindows) {
    # Poetry uses `SHELL` to determine the shell to use for activate.
    # Set it to PowerShell if not already set.
    $env:SHELL = "pwsh"
}

# Check for dependencies we don't automatically install
& "$RepoRoot/script/check-deps.ps1"

Write-Host "Installing Python build tools..."
pip install maturin
pip install poetry
poetry config virtualenvs.in-project true

Write-Host "Installing Python dependencies..."
Push-Location $RepoRoot/python
try {
    Invoke-Expression (poetry env activate)
    poetry install
}
finally { 
    Pop-Location
}

Write-Host "Installing Rust dependencies..."
cargo install --locked cbindgen@0.29.0
cargo install --locked just@1.42.4

Write-Host "Installing addlicense..."
go install github.com/google/addlicense@v1.1.1