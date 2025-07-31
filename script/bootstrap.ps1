$RepoRoot = Split-Path $PSScriptRoot -Parent

# Check for dependencies we don't automatically install
& "$RepoRoot/script/check-deps.ps1"

# Dump the path variable?
Write-Host "Current PATH: $env:PATH"

# Current Python
$pythonPath = py -c "import sys; print(sys.executable)"
Write-Host "Current Python: $pythonPath"

# Current Pip
$pipPath = py -c "import sys; print(sys.exec_prefix)"
Write-Host "Current Pip: $pipPath"

Write-Host "Install pipx"
py -m pip install --user pipx

Write-Host "Installing Python build tools..."
pipx install maturin
pipx install poetry
poetry config virtualenvs.in-project true

Write-Host "Installing Rust dependencies..."
cargo install --locked cbindgen@0.29.0

Write-Host "Installing Python dependencies..."
Invoke-Expression (poetry -C ./python env activate)
poetry -C ./python install

Write-Host "Installing addlicense..."
go install github.com/google/addlicense@v1.1.1