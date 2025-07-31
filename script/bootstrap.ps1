$RepoRoot = Split-Path $PSScriptRoot -Parent

# Check for dependencies we don't automatically install
& "$RepoRoot/script/check-deps.ps1"

# Dump the path variable?
Write-Host "Current PATH: $env:PATH"

# Current Python
$pythonPath = python -c "import sys; print(sys.executable)"
Write-Host "Current Python: $pythonPath"

# Current Pip
$pipPath = python -c "import sys; print(sys.exec_prefix)"
Write-Host "Current Pip: $pipPath"

# Current Python Version
$pythonVersion = python -c "import sys; print(sys.version)"
Write-Host "Current Python Version: $pythonVersion"

Write-Host "Install pipx"
python -m pip install pipx

Write-Host "Installing Python build tools..."
pipx install --python $env:PY_PYTHON maturin
pipx install --python $env:PY_PYTHON poetry
poetry config virtualenvs.in-project true

Write-Host "Installing Rust dependencies..."
cargo install --locked cbindgen@0.29.0

Write-Host "Installing Python dependencies..."
Invoke-Expression (poetry -C ./python env activate)
poetry -C ./python install

Write-Host "Installing addlicense..."
go install github.com/google/addlicense@v1.1.1