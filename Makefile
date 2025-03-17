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

target_dir := $(target_root)/$(CARGO_BUILD_TARGET)/$(CONFIGURATION)
artifacts_dir := $(artifacts_root)/$(CARGO_BUILD_TARGET)/$(CONFIGURATION)

shared_lib_name := cosmoscx

cosmoscx_header_name := $(shared_lib_name).h

COSMOSCX_HEADER_PATH ?= $(root_dir)/include/$(shared_lib_name).h

LIBRARY_MODE ?= static

ifeq ($(OS),Windows_NT)
	platform := windows
else
	uname := $(shell uname)
	ifeq ($(uname),Darwin)
		platform := macos
	else
		platform := linux
	endif
endif

ifeq ($(platform),windows)
	PATH := $(artifacts_dir)/lib:$(PATH)
	ifeq ($(CARGO_BUILD_TARGET), x86_64-pc-windows-gnu)
		shared_lib_filename := $(shared_lib_name).dll
		static_lib_filename := lib$(shared_lib_name).a
	else
		shared_lib_filename := $(shared_lib_name).dll
		static_lib_filename := $(shared_lib_name).lib
	endif
else ifeq ($(platform),macos)
	shared_lib_filename := lib$(shared_lib_name).dylib
	static_lib_filename := lib$(shared_lib_name).a
else
	shared_lib_filename := lib$(shared_lib_name).so
	static_lib_filename := lib$(shared_lib_name).a
endif

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
	LD_LIBRARY_PATH := $(artifacts_dir)/lib:$(LD_LIBRARY_PATH)
	DYLD_LIBRARY_PATH := $(artifacts_dir)/lib:$(DYLD_LIBRARY_PATH)
endif

export LD_LIBRARY_PATH
export DYLD_LIBRARY_PATH

# Default target, don't put any targets above this one.
.PHONY: all
all: headers engine test #/ Builds the engine and runs all tests

.PHONY: help
help: #/ Show this help
	@egrep -h '^#/\s' $(MAKEFILE_LIST) | sed -e 's/^#\/\s*//'
	@echo ""
	@echo "Targets:"
	@egrep -h '\s#/\s' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?#/ *"}; {printf "\033[36m%-30s\033[0m %s\n", $$1, $$2}'

.PHONY: engine
engine: engine_rust engine_c engine_python #/ Builds all versions of the engine

.PHONY: headers
headers: #/ Builds the C header file for the engine, used by cgo and other bindgen-like tools
	cbindgen --quiet --config cbindgen.toml --crate "cosmoscx" --output $(COSMOSCX_HEADER_PATH)

.PHONY: engine_rust
engine_rust: #/ Builds the Core Rust Engine.
	cargo build --package "azure_data_cosmos_engine" --profile $(cargo_profile)

.PHONY: engine_c
engine_c: #/ Builds the C API for the engine, producing the shared and static libraries
	cargo build --package "cosmoscx" --profile $(cargo_profile)
	cargo rustc --package "cosmoscx" -- --print native-static-libs
	mkdir -p $(artifacts_dir)/lib
	ls -l $(target_dir)
	cp $(target_dir)/$(shared_lib_filename) $(artifacts_dir)/lib/$(shared_lib_filename)
	cp $(target_dir)/$(static_lib_filename) $(artifacts_dir)/lib/$(static_lib_filename)
	script/helpers/update-dylib-name $(artifacts_dir)/lib/$(shared_lib_filename)
	script/helpers/write-pkg-config.sh $(artifacts_dir) $(root_dir)/include

.PHONY: engine_python
engine_python: #/ Builds the python extension module for the engine
	poetry -C ./python run maturin develop --profile $(cargo_profile) $(maturin_args)

.PHONY: test
test: test_rust test_go #/ Runs all language binding tests, except Python which isn't currently supported

.PHONY: test_rust
test_rust:
	RUSTFLAGS=$(TEST_RUSTFLAGS) cargo test --profile $(cargo_profile) --workspace --all-features
	cargo doc --profile $(cargo_profile) --no-deps --workspace --all-features

.PHONY: test_go
test_go: #/ Runs the Go language binding tests
	@echo "Running Go tests..."
	go -C ./go/azcosmoscx test -tags "$(GOTAGS)" -v ./...

.PHONY: test_python
test_python:
	@echo "Running Python tests..."
	poetry -C ./python run python -m pytest -rP .

integration_test_go: #/ Runs the Go language binding integration tests
	@echo "Running Go integration tests..."
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

show_pkg_config: #/ Shows the pkg-config settings for the library under the current settings
	@echo "cflags: $$(pkg-config --cflags cosmoscx)"
	@echo "libs: $$(pkg-config --libs cosmoscx)"

vendor: engine_c #/ Updates the vendored copy of the library
	mkdir -p $(root_dir)/go/azcosmoscx/libcosmoscx-vendor/$(CARGO_BUILD_TARGET)
	cp $(artifacts_dir)/lib/$(static_lib_filename) $(root_dir)/go/azcosmoscx/libcosmoscx-vendor/$(CARGO_BUILD_TARGET)/$(static_lib_filename)