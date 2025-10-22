# Cross platform powershell shebang
shebang := if os() == 'windows' {
  '#!pwsh -ExecutionPolicy Bypass -File'
} else {
  '#!/usr/bin/env pwsh'
}

set shell := ["pwsh", "-c"]
set windows-shell := ["pwsh", "-NoLogo", "-Command"]

# NOTE: Just automatically sets the working directory to the location of the Justfile, so we can always assume we're at the repo root.

import 'script/just/targets.just'
import 'script/just/variables.just'

# Attach the appropriate linker if cross-compiling.
export RUSTFLAGS := if linker != "" {
  "-C linker=" + linker + " " + env("RUSTFLAGS", "")
} else {
  env("RUSTFLAGS", "")
}

# Updates the headers, builds the engine, and runs unit tests.
default: headers engine test

# Generates the C header file for cosmoscx.
headers: _generate_headers
  cp {{ header_path }} ./go/azcosmoscx/include/cosmoscx.h

_generate_headers:
  cbindgen --quiet --config cbindgen.toml --crate "cosmoscx" --output {{ header_path }}

# Builds the Rust engine and C wrapper.
engine: engine_rust engine_c

# Builds the Rust engine.
engine_rust:
  cargo build --package "azure_data_cosmos_engine" --profile {{ cargo_profile }}

# Builds the Python wrapper around the Rust engine. (Currently disabled)
engine_python:
  poetry -C ./python run maturin develop --profile {{ cargo_profile }} --target {{ build_target }}

_copy_import_command := if import_lib_filename != "" {
  "Copy-Item " + target_dir / import_lib_filename + " " + artifacts_dir / "lib" / import_lib_filename
} else {
  ""
}

# Builds the C wrapper around the Rust engine.
engine_c:
  cargo build --package "cosmoscx" --profile {{ cargo_profile }}
  if(-not (Test-Path {{artifacts_dir}}/lib)) { New-Item -Type Directory {{artifacts_dir}}/lib }
  Get-ChildItem {{target_dir}}
  Copy-Item {{target_dir}}/{{shared_lib_filename}} {{artifacts_dir}}/lib/{{shared_lib_filename}}
  Copy-Item {{target_dir}}/{{static_lib_filename}} {{artifacts_dir}}/lib/{{static_lib_filename}}
  {{ _copy_import_command }}
  & script/helpers/update-dylib-name.ps1 -TargetOS {{target_os}} -DylibPath {{target_dir}}/{{shared_lib_filename}}
  & script/helpers/write-pkg-config.ps1 -Prefix {{artifacts_dir}} -Includedir {{artifacts_dir}}/include

# Displays the native libraries required by the C wrapper in the current build configuration.
print-native-libraries:
	cargo rustc --package "cosmoscx" --profile {{cargo_profile}} -- --print native-static-libs

# Vendors the static library into the Go wrapper.
vendor: engine_c
  if(-not (Test-Path ./go/azcosmoscx/libcosmoscx-vendor/{{ build_target }})) { New-Item -Type Directory ./go/azcosmoscx/libcosmoscx-vendor/{{ build_target }} }
  Copy-Item {{artifacts_dir}}/lib/{{static_lib_filename}} ./go/azcosmoscx/libcosmoscx-vendor/{{ build_target }}

# Tests the Rust engine and Go wrapper.
test: test_rust test_go

# Tests the Rust engine.
test_rust:
  # We don't use '--all-features' because the 'python_conversions' feature depends on libpython, which is not available unless we're building with maturin.
  cargo test --profile {{ cargo_profile }} --package "azure_data_cosmos_engine" --package cosmoscx
  cargo doc --profile {{ cargo_profile }} --no-deps --workspace

# Tests the Python wrapper around the Rust engine. (Currently disabled)
test_python:
  poetry -C ./python run python -m pytest -rP .

# Tests the Go wrapper around the Rust engine.
test_go:
  go -C ./go/azcosmoscx clean -testcache
  go -C ./go/azcosmoscx test -tags {{ go_tags }} -v ./...

# Runs end-to-end query tests for the Rust engine and Go wrapper.
query_test: query_test_rust query_test_go

# Runs end-to-end query tests for the Rust engine.
query_test_rust test="":
  cargo test --profile {{ cargo_profile }} --package query-tests -- --test-threads 1 {{ test }}

# Runs end-to-end query tests for the Python wrapper. (Currently disabled)
query_test_python:
  poetry -C ./python run python -m pytest -rP ./test/query-tests

# Runs end-to-end query tests for the Go wrapper.
query_test_go:
  go -C ./go/integration-tests clean -testcache
  go -C ./go/integration-tests test -tags {{ go_tags }} -v ./...

# Cleans up build artifacts and caches.
clean:
  go -C ./go/azcosmoscx clean -cache
  go -C ./go/integration-tests clean -cache
  cargo clean --profile {{ cargo_profile }}
  rm -rf {{ artifacts_dir }}

# Deletes the artifacts and target directories, including all build artifacts and caches.
superclean: clean
  rm -rf {{ artifacts_root }}
  rm -rf {{ target_root }}

# Updates query end-to-end test baselines
baselines:
  dotnet run --project ./baselines/baseline-generator/baseline-generator.csproj -- ./baselines/queries

# Runs formatting and linting checks without making changes.
check: (_fmt "check")

# Runs formatting and linting checks, and automatically fixes issues.
fix: (_fmt "fix")

_fmt fix:
  cargo fmt --all {{ if fix == "fix" { "" } else { "--check" } }}
  addlicense {{ if fix == "fix" { "" } else { "-check" } }} \
    -f "./script/licenseheader.tpl" \
    -ignore '**/*.yml' \
    -ignore '**/obj/**' \
    -ignore 'include/*.h' \
    -ignore 'target/**' \
    -ignore 'script/**' \
    -ignore 'artifacts/**' \
    -ignore 'python/.venv/**' \
    .
