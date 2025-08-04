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