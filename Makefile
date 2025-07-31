# We don't really use the dependency management in this Makefile much because we have fairly complicated cross-language dependencies.
# We just hope that most of the tools we invoke can do incremental compilations.

# NOTE: Any line that starts '#/' will appear in the help output when running 'make help'.
# In addition, any '#/' comment that appears after a target will be used to describe that target in the help output.

#/ Makefile for the Azure Data Cosmos Client Engine

root_dir := $(shell git rev-parse --show-toplevel)
CONFIGURATION ?= debug
target_root ?= $(root_dir)/target
artifacts_root ?= $(root_dir)/artifacts

# NOTE: It's safe to have a trailing or single ',' in the '-tags' parameter we pass to go tags.
ifneq ($(VENDORED),true)
	GOTAGS ?= azcosmoscx_local
endif

# If CARGO_BUILD_TARGET is not set, we'll use the host target.
export CARGO_BUILD_TARGET ?= $(shell rustc -vV | grep 'host: ' | cut -d ' ' -f 2)

# Clunky, but the easiest way to "parse" a target triple.
ifeq ($(CARGO_BUILD_TARGET),x86_64-unknown-linux-gnu)
	TARGET_OS := linux
	TARGET_ARCH := x86_64
	TARGET_TOOLCHAIN := gnu
else ifeq ($(CARGO_BUILD_TARGET),aarch64-unknown-linux-gnu)
	TARGET_OS := linux
	TARGET_ARCH := aarch64
	TARGET_TOOLCHAIN := gnu
else ifeq ($(CARGO_BUILD_TARGET),x86_64-pc-windows-gnu)
	TARGET_OS := windows
	TARGET_ARCH := x86_64
	TARGET_TOOLCHAIN := gnu
else ifeq ($(CARGO_BUILD_TARGET),x86_64-pc-windows-msvc)
	TARGET_OS := windows
	TARGET_ARCH := x86_64
	TARGET_TOOLCHAIN := msvc
else ifeq ($(CARGO_BUILD_TARGET),aarch64-apple-darwin)
	TARGET_OS := macos
	TARGET_ARCH := aarch64
	TARGET_TOOLCHAIN := apple
else ifeq ($(CARGO_BUILD_TARGET),x86_64-apple-darwin)
	TARGET_OS := macos
	TARGET_ARCH := x86_64
	TARGET_TOOLCHAIN := apple
else
	TARGET_OS := $(error Unsupported target '$(CARGO_BUILD_TARGET)')
endif

target_dir := $(target_root)/$(CARGO_BUILD_TARGET)/$(CONFIGURATION)
artifacts_dir := $(artifacts_root)/$(CARGO_BUILD_TARGET)/$(CONFIGURATION)

shared_lib_name := cosmoscx

cosmoscx_header_name := $(shared_lib_name).h

COSMOSCX_HEADER_PATH ?= $(root_dir)/include/$(shared_lib_name).h

LIBRARY_MODE ?= static

ifeq ($(TARGET_OS),windows)
	PATH := $(artifacts_dir)/lib:$(PATH)
	ifeq ($(TARGET_TOOLCHAIN), gnu)
		shared_lib_filename := $(shared_lib_name).dll
		import_lib_filename := $(shared_lib_name).dll.a
		static_lib_filename := lib$(shared_lib_name).a
	else
		shared_lib_filename := $(shared_lib_name).dll
		import_lib_filename := $(shared_lib_name).dll.lib
		static_lib_filename := $(shared_lib_name).lib
	endif
else ifeq ($(TARGET_OS),macos)
	shared_lib_filename := lib$(shared_lib_name).dylib
	static_lib_filename := lib$(shared_lib_name).a
else
	shared_lib_filename := lib$(shared_lib_name).so
	static_lib_filename := lib$(shared_lib_name).a
	strip_args := --strip-debug
endif

# Configure pkg-config on macOS and Linux to find the artifacts directory.
PKG_CONFIG_PATH := $(artifacts_dir):$(PKG_CONFIG_PATH)

export PATH
export PKG_CONFIG_PATH

# Cargo calls the 'debug' configuration 'dev', yet it still builds to a 'debug' directory in the target directory.
ifeq ($(CONFIGURATION),debug)
	cargo_profile = dev
	GOTAGS := debug,$(GOTAGS)
else
	cargo_profile = $(CONFIGURATION)
endif

ifeq ($(LIBRARY_MODE),shared)
	GOTAGS := dynamic,$(GOTAGS)
	ifeq ($(TARGET_OS),windows)
		# On Windows, we use the PATH environment variable to find the shared library.
		# And we need to set the CGO_LDFLAGS to point to the artifacts directory, because Windows doesn't use pkg-config.
		PATH := $(artifacts_dir)/lib:$(PATH)
		CGO_LDFLAGS := -L$(artifacts_dir)/lib
	else ifeq ($(TARGET_OS),macos)
		# On macOS, we use DYLD_LIBRARY_PATH to find the shared library.
		DYLD_LIBRARY_PATH := $(artifacts_dir)/lib:$(DYLD_LIBRARY_PATH)
	else
		# On Linux, we use LD_LIBRARY_PATH to find the shared library.
		LD_LIBRARY_PATH := $(artifacts_dir)/lib:$(LD_LIBRARY_PATH)
	endif
endif

export LD_LIBRARY_PATH
export DYLD_LIBRARY_PATH

# Default target, don't put any targets above this one.
.PHONY: all
all: headers engine test check #/ Builds the engine and runs all tests

.PHONY: help
help: #/ Show this help
	@egrep -h '^#/\s' $(MAKEFILE_LIST) | sed -e 's/^#\/\s*//'
	@echo ""
	@echo "Targets:"
	@egrep -h '\s#/\s' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?#/ *"}; {printf "\033[36m%-30s\033[0m %s\n", $$1, $$2}'

# disabled engine_python for now, as it's not currently supported.
.PHONY: engine
engine: engine_rust engine_c #/ Builds all versions of the engine

.PHONY: _generate_headers
_generate_headers: #/ (Internal) Generates the header file for the engine.
	cbindgen --quiet --config cbindgen.toml --crate "cosmoscx" --output $(COSMOSCX_HEADER_PATH)

.PHONY: headers
headers: _generate_headers #/ Builds the C header file for the engine, used by cgo and other bindgen-like tools
	# There needs to be a copy inside the Go package for cgo to find it.
	[ -d $(root_dir)/go/azcosmoscx/include ] || mkdir -p $(root_dir)/go/azcosmoscx/include
	cp $(COSMOSCX_HEADER_PATH) $(root_dir)/go/azcosmoscx/include/cosmoscx.h

	# There needs to be a copy inside the Go package for cgo to find it.
	[ -d $(root_dir)/go/azcosmoscx/include ] || mkdir -p $(root_dir)/go/azcosmoscx/include
	cp $(COSMOSCX_HEADER_PATH) $(root_dir)/go/azcosmoscx/include/cosmoscx.h

.PHONY: engine_rust
engine_rust: #/ Builds the Core Rust Engine.
	cargo build --package "azure_data_cosmos_engine" --profile $(cargo_profile)

.PHONY: engine_c
engine_c: #/ Builds the C API for the engine, producing the shared and static libraries
	cargo build --package "cosmoscx" --profile $(cargo_profile)
	mkdir -p $(artifacts_dir)/lib
	ls -l $(target_dir)
	cp $(target_dir)/$(shared_lib_filename) $(artifacts_dir)/lib/$(shared_lib_filename)
	cp $(target_dir)/$(static_lib_filename) $(artifacts_dir)/lib/$(static_lib_filename)
	[ -n "$(import_lib_filename)" ] && cp $(target_dir)/$(import_lib_filename) $(artifacts_dir)/lib/$(import_lib_filename)
	script/helpers/update-dylib-name $(artifacts_dir)/lib/$(shared_lib_filename)
	script/helpers/write-pkg-config.sh $(artifacts_dir) $(root_dir)/include

.PHONE: _print-native-libraries
_print-native-libraries: #/ (Internal) Prints the native libraries that will be used by
	cargo rustc --package "cosmoscx" --profile $(cargo_profile) -- --print native-static-libs

.PHONY: engine_python
engine_python: #/ Builds the python extension module for the engine
	poetry -C ./python run maturin develop --profile $(cargo_profile) $(maturin_args)

.PHONY: test
test: test_rust test_go #/ Runs all language binding tests, except Python which isn't currently supported

.PHONY: test_rust
test_rust:
	RUSTFLAGS=$(TEST_RUSTFLAGS) cargo test --profile $(cargo_profile) --package azure_data_cosmos_engine --package cosmoscx
	cargo doc --profile $(cargo_profile) --no-deps --workspace

.PHONY: test_go
test_go: #/ Runs the Go language binding tests
	@echo "Running Go tests..."
	go -C ./go/azcosmoscx clean -testcache
	go -C ./go/azcosmoscx test -tags "$(GOTAGS)" -v ./...

.PHONY: test_python
test_python: #/ Runs the Python language binding tests
	@echo "Running Python tests..."
	poetry -C ./python run python -m pytest -rP .

.PHONY: query_test
query_test: query_test_rust query_test_go query_test_python #/ Runs all query tests

.PHONY: query_test_python
query_test_python: #/ Runs the Python query tests
	@echo "Running Python query tests..."
	poetry -C ./python run python -m pytest -rP ./test/query-tests

.PHONY: query_test_rust
query_test_rust: #/ Runs the Rust query tests
	@echo "Running Rust query tests..."
	RUSTFLAGS=$(TEST_RUSTFLAGS) cargo test --profile $(cargo_profile) --package query-tests

.PHONY: query_test_go
query_test_go: #/ Runs the Go language binding integration tests
	@echo "Running Go integration tests..."
	go -C ./go/azcosmoscx clean -testcache
	go -C ./go/integration-tests test -tags "$(GOTAGS)" -v ./...

.PHONY: superclean
superclean: clean #/ Delete the entire `targets` and `artifacts` directories
	rm -rf $(target_root) $(artifacts_root)

.PHONY: clean
clean: clean_go clean_rust clean_artifacts #/ Cleans all build artifacts

.PHONY: clean_go
clean_go: #/ Cleans all Go build artifacts
	go -C ./go/azcosmoscx clean -cache

.PHONY: clean_rust
clean_rust: #/ Cleans all Rust build artifacts
	cargo clean --profile $(cargo_profile)

.PHONY: clean_artifacts
clean_artifacts: #/ Cleans the artifacts directory, which contains the generated C headers and libraries
	rm -rf $(artifacts_dir)

.PHONY: show_pkg_config
show_pkg_config: #/ Shows the pkg-config settings for the library under the current settings
	@echo "cflags: $$(pkg-config --cflags cosmoscx)"
	@echo "libs: $$(pkg-config --libs cosmoscx)"

.PHONY: vendor
vendor: engine_c #/ Updates the vendored copy of the library
	mkdir -p $(root_dir)/go/azcosmoscx/libcosmoscx-vendor/$(CARGO_BUILD_TARGET)
	cp $(artifacts_dir)/lib/$(static_lib_filename) $(root_dir)/go/azcosmoscx/libcosmoscx-vendor/$(CARGO_BUILD_TARGET)/$(static_lib_filename)
	strip $(strip_args) $(root_dir)/go/azcosmoscx/libcosmoscx-vendor/$(CARGO_BUILD_TARGET)/$(static_lib_filename)

.PHONY: baselines
baselines: #/ Updates query result baselines using the emulator and the .NET client.
	dotnet run --project ./baselines/baseline-generator/baseline-generator.csproj -- ./baselines/queries

.PHONY: check
check: #/ Run linters and formatters in check mode
	@echo "Running clippy..."
	@cargo clippy --workspace --all-targets --all-features -- -D warnings
	@if ! script/fmt --check; then \
		echo "Formatting errors found. Run 'script/fmt --fix' to fix formatting issues."; \
		exit 1; \
	fi
