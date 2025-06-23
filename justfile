# Justfile for the Azure Data Cosmos Client Engine
# This replaces the original Makefile with a more modern, cross-platform build system

# Variables
root_dir := `git rev-parse --show-toplevel# Run Go tests with tags
_go_test tags:
    cd {{root_dir}}/go/azcosmoscx && go test -tags "{{tags}}" -v ./...

# Run Rust tests with RUSTFLAGS
_cargo_test *args:
    RUSTFLAGS={{env_var_or_default("TEST_RUSTFLAGS", "")}} cargo test --profile {{cargo_profile}} {{args}}

# Run addlicense with given command and parameters
_addlicense command:
    @if ! command -v addlicense >/dev/null 2>&1; then \
        echo "Warning: addlicense not found, skipping license header {{command}}"; \
        echo "Install with: go install github.com/google/addlicense@latest"; \
    else \
        addlicense {{command}} -f {{root_dir}}/script/licenseheader.tpl {{addlicense_ignores}} {{root_dir}}; \
    fionfiguration := env_var_or_default("CONFIGURATION", "debug")
target_root := root_dir / "target"
artifacts_root := root_dir / "artifacts"
vendored := env_var_or_default("VENDORED", "false")

# Platform detection
platform := if os() == "windows" { "windows" } else if os() == "macos" { "macos" } else { "linux" }

# Cargo build target
CARGO_BUILD_TARGET := `rustc -vV | grep 'host:' | cut -d ' ' -f 2`
export CARGO_BUILD_TARGET

# Derived paths
target_dir := target_root / CARGO_BUILD_TARGET / configuration
artifacts_dir := artifacts_root / CARGO_BUILD_TARGET / configuration

# Cargo profile mapping (cargo uses 'dev' for debug)
cargo_profile := if configuration == "debug" { "dev" } else { configuration }

# Go build tags
gotags := if vendored == "true" { if configuration == "debug" { "debug" } else { "" } } else { if configuration == "debug" { "azcosmoscx_local,debug" } else { "azcosmoscx_local" } }

# Library names and paths
shared_lib_name := "cosmoscx"
cosmoscx_header_path := root_dir / "include" / shared_lib_name + ".h"
python_project_dir := root_dir / "python"
build_utils_script := root_dir / "script/helpers/build_utils.py"

# Library mode
library_mode := env_var_or_default("LIBRARY_MODE", "static")

# Addlicense ignore patterns
addlicense_ignores := "-ignore '**/*.yml' -ignore 'include/*.h' -ignore 'target/**' -ignore 'script/**' -ignore 'artifacts/**' -ignore 'python/.venv/**' -ignore '.venv/**' -ignore '**/venv/**' -ignore '**/.venv/**' -ignore '**/site-packages/**' -ignore '**/bin/**' -ignore '**/obj/**' -ignore '**/.git/**' -ignore '**/doc/**' -ignore '*.lock' -ignore '**/Cargo.lock'"

# Environment setup for Go
export PKG_CONFIG_PATH := artifacts_dir + ":" + env_var_or_default("PKG_CONFIG_PATH", "")

# Platform-specific library path setup  
export PATH := if platform == "windows" { artifacts_dir / "lib" + ":" + env_var_or_default("PATH", "") } else { env_var_or_default("PATH", "") }
export LD_LIBRARY_PATH := if library_mode == "shared" { artifacts_dir / "lib" + ":" + env_var_or_default("LD_LIBRARY_PATH", "") } else { env_var_or_default("LD_LIBRARY_PATH", "") }
export DYLD_LIBRARY_PATH := if library_mode == "shared" { artifacts_dir / "lib" + ":" + env_var_or_default("DYLD_LIBRARY_PATH", "") } else { env_var_or_default("DYLD_LIBRARY_PATH", "") }

# Default target
default: headers engine test check

# Show help
help:
    @echo "Justfile for the Azure Data Cosmos Client Engine"
    @echo ""
    @echo "Available recipes:"
    @just --list

# Build all versions of the engine
engine: engine_rust engine_c engine_python

# Build the C header file for the engine
headers:
    @echo "Building C headers..."
    cbindgen --quiet --config cbindgen.toml --crate "cosmoscx" --output {{cosmoscx_header_path}}
    # Copy header to Go package for cgo
    @just _ensure_go_include_dir
    cp {{cosmoscx_header_path}} {{root_dir}}/go/azcosmoscx/include/cosmoscx.h

# Build the Core Rust Engine
engine_rust:
    @echo "Building Rust engine..."
    cargo build --package "azure_data_cosmos_engine" --profile {{cargo_profile}}

# Build the C API for the engine
engine_c:
    @echo "Building C API..."
    cargo build --package "cosmoscx" --profile {{cargo_profile}}
    cargo rustc --package "cosmoscx" -- --print native-static-libs
    @echo "Copying artifacts..."
    python3 {{build_utils_script}} copy-artifacts {{target_root}} {{artifacts_dir}} {{CARGO_BUILD_TARGET}} {{configuration}}
    python3 {{build_utils_script}} write-pkg-config {{artifacts_dir}} {{root_dir}}/include

# Build the Python extension module
engine_python:
    @echo "Building Python extension..."
    poetry -C {{python_project_dir}} run maturin develop --profile {{cargo_profile}}

# Run all tests
test: test_rust test_go

# Run Rust tests
test_rust:
    @echo "Running Rust tests..."
    @just _cargo_test --package azure_data_cosmos_engine --package cosmoscx --all-features
    cargo doc --profile {{cargo_profile}} --no-deps --workspace --all-features

# Run Go tests
test_go:
    @echo "Running Go tests..."
    @just _go_clean_testcache
    @just _go_test "{{gotags}}"

# Run Python tests
test_python:
    @echo "Running Python tests..."
    poetry -C {{python_project_dir}} run python -m pytest -rP .

# Run all query tests
query_test: query_test_rust query_test_go query_test_python

# Run Python query tests
query_test_python:
    @echo "Running Python query tests..."
    poetry -C {{python_project_dir}} run python -m pytest -rP ./test/query-tests

# Run Rust query tests
query_test_rust:
    @echo "Running Rust query tests..."
    @just _cargo_test --package query-tests

# Run Go integration tests
query_test_go:
    @echo "Running Go integration tests..."
    @just _go_clean_testcache
    cd {{root_dir}}/go/integration-tests && go test -tags "{{gotags}}" -v ./...

# Delete the entire targets and artifacts directories
superclean: clean
    @echo "Super cleaning..."
    rm -rf {{target_root}} {{artifacts_root}}

# Clean all build artifacts
clean: clean_go clean_rust clean_artifacts

# Clean Go build artifacts
clean_go:
    @echo "Cleaning Go artifacts..."
    cd {{root_dir}}/go/azcosmoscx && go clean -cache

# Clean Rust build artifacts
clean_rust:
    @echo "Cleaning Rust artifacts..."
    cargo clean --profile {{cargo_profile}}

# Clean the artifacts directory
clean_artifacts:
    @echo "Cleaning artifacts directory..."
    rm -rf {{artifacts_dir}}

# Show pkg-config settings
show_pkg_config:
    @echo "cflags: $(pkg-config --cflags cosmoscx)"
    @echo "libs: $(pkg-config --libs cosmoscx)"

# Update the vendored copy of the library
vendor: engine_c
    @echo "Updating vendored library..."
    @just _copy_vendor_lib

# Update query result baselines
baselines:
    @echo "Updating baselines..."
    dotnet run --project {{root_dir}}/baselines/baseline-generator/baseline-generator.csproj -- {{root_dir}}/baselines/queries

# Run linters and formatters in check mode
check:
    @echo "Running clippy..."
    cargo clippy --workspace --all-targets --all-features -- -D warnings
    @echo "Checking formatting..."
    @just _fmt_check

# Fix formatting issues
fmt-fix:
    @echo "Fixing formatting..."
    @just _fmt_fix

# Check formatting without fixing
fmt-check:
    @echo "Checking formatting..."
    @just _fmt_check

# Private recipes (prefixed with _)

# Ensure Go include directory exists
_ensure_go_include_dir:
    @mkdir -p {{root_dir}}/go/azcosmoscx/include

# Copy library to vendor directory
_copy_vendor_lib:
    python3 {{build_utils_script}} copy-vendor-lib {{artifacts_dir}} {{root_dir}} {{CARGO_BUILD_TARGET}}

# Clean Go test cache
_go_clean_testcache:
    cd {{root_dir}}/go/azcosmoscx && go clean -testcache

# Run Go tests with tags
_go_test tags:
    cd {{root_dir}}/go/azcosmoscx && go test -tags "{{tags}}" -v ./...

# Check formatting without fixing
_fmt_check:
    @echo "Running Rust formatter in check mode..."
    cargo fmt --all --check
    @echo "Running addlicense in check mode..."
    @just _addlicense "-check"

# Fix formatting issues
_fmt_fix:
    @echo "Running Rust formatter..."
    cargo fmt --all
    @echo "Running addlicense..."
    @just _addlicense ""

# Print current configuration
config:
    @echo "Current configuration:"
    @echo "  Platform: {{platform}}"
    @echo "  Configuration: {{configuration}}"
    @echo "  Cargo profile: {{cargo_profile}}"
    @echo "  Cargo build target: {{CARGO_BUILD_TARGET}}"
    @echo "  Go tags: {{gotags}}"
    @echo "  Library mode: {{library_mode}}"
    @echo "  Root dir: {{root_dir}}"
    @echo "  Target dir: {{target_dir}}"
    @echo "  Artifacts dir: {{artifacts_dir}}"
