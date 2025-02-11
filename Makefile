# We don't really use the dependency management in this Makefile much because we have fairly complicated cross-language dependencies.
# We just hope that most of the tools we invoke can do incremental compilations.

# NOTE: Any line that starts '#/' will appear in the help output when running 'make help'.
# In addition, any '#/' comment that appears after a target will be used to describe that target in the help output.

#/ Makefile for the Azure Data Cosmos Client Engine

root_dir := $(shell git rev-parse --show-toplevel)
CONFIGURATION ?= debug
target_root ?= $(root_dir)/target
artifacts_root ?= $(root_dir)/artifacts

# If CARGO_BUILD_TARGET is not set, we'll use the host target.
export CARGO_BUILD_TARGET ?= $(shell rustc -vV | grep 'host: ' | cut -d ' ' -f 2)

target_dir := $(target_root)/$(CARGO_BUILD_TARGET)/$(CONFIGURATION)
artifacts_dir := $(artifacts_root)/$(CARGO_BUILD_TARGET)/$(CONFIGURATION)

crate_name := azure_cosmoscx
shared_lib_name := cosmoscx

cosmoscx_header_name := $(shared_lib_name).h

COSMOSCX_HEADER_PATH ?= $(root_dir)/include/$(shared_lib_name).h

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
	compiled_shared_lib_filename := $(crate_name).dll
	compiled_static_lib_filename := $(crate_name).lib
	shared_lib_filename := $(shared_lib_name).dll
	static_lib_filename := $(shared_lib_name).lib
else ifeq ($(platform),macos)
	compiled_shared_lib_filename := lib$(crate_name).dylib
	compiled_static_lib_filename := lib$(crate_name).a
	shared_lib_filename := lib$(shared_lib_name).dylib
	static_lib_filename := lib$(shared_lib_name).a
else
	compiled_shared_lib_filename := lib$(crate_name).so
	compiled_static_lib_filename := lib$(crate_name).a
	shared_lib_filename := lib$(shared_lib_name).so
	static_lib_filename := lib$(shared_lib_name).a
endif

# Set linker flags for building and testing
ifeq ($(LIBRARY_MODE),static)
	CGO_LDFLAGS := $(artifacts_dir)/lib/$(static_lib_filename)
else
	CGO_LDFLAGS := -L$(artifacts_dir)/lib -l$(shared_lib_name) -Wl,-rpath,$(artifacts_dir)/lib
	LD_LIBRARY_PATH := $(artifacts_dir)/lib:$(LD_LIBRARY_PATH)
endif

ifeq ($(platform),macos)
	# These flags are required when building the Python feature on macOS.
	# Maturin does this for us, but when we build the tests we need to set them.
	TEST_RUSTFLAGS := "-C link-arg=-undefined -C link-arg=dynamic_lookup"
endif

export CGO_LDFLAGS
export LD_LIBRARY_PATH
export PATH

# Cargo calls the 'debug' configuration 'dev', yet it still builds to a 'debug' directory in the target directory.
ifeq ($(CONFIGURATION),debug)
	cargo_profile = dev
else
	cargo_profile = $(CONFIGURATION)
endif

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
engine: engine_c engine_python #/ Builds all versions of the engine

.PHONY: headers
headers: #/ Builds the C header file for the engine, used by cgo and other bindgen-like tools
	cbindgen --quiet --config cbindgen.toml --crate $(crate_name) --output $(COSMOSCX_HEADER_PATH)

.PHONY: engine_c
engine_c: #/ Builds the C API for the engine, producing the shared and static libraries
	cargo build --package $(crate_name) --features c_api --profile $(cargo_profile)
	mkdir -p $(artifacts_dir)/lib
	cp $(target_dir)/$(compiled_shared_lib_filename) $(artifacts_dir)/lib/$(shared_lib_filename)
	cp $(target_dir)/$(compiled_static_lib_filename) $(artifacts_dir)/lib/$(static_lib_filename)
	script/helpers/update-dylib-name $(artifacts_dir)/lib/$(shared_lib_filename)

.PHONY: engine_python
engine_python: _check-venv #/ Builds the python extension module for the engine
	cd "$(root_dir)/python" && maturin develop --profile $(cargo_profile) $(maturin_args)

.PHONY: test
test: test_go test_python #/ Runs all language binding tests

.PHONY: test_rust
test_rust:
	RUSTFLAGS=$(TEST_RUSTFLAGS) cargo test --profile $(cargo_profile) --workspace --all-features
	cargo doc --profile $(cargo_profile) --no-deps --workspace --all-features

.PHONY: test_go
test_go: #/ Runs the Go language binding tests
	@echo "Running Go tests..."
	go -C ./go/azcosmoscx test -v ./...

.PHONY: test_python
test_python: _check-venv #/ Runs the Python language binding tests
	@echo "Running Python tests..."
	python -m pytest ./python

.PHONY: superclean
superclean: #/ Delete the entire `targets` and `artifacts` directories
	rm -Rf $(target_root) $(artifacts_root)

.PHONY: clean
clean: clean_rust clean_artifacts #/ Cleans all build artifacts

.PHONY: clean_rust
clean_rust: #/ Cleans all Rust build artifacts
	cargo clean --profile $(cargo_profile)

.PHONY: clean_artifacts
clean_artifacts: #/ Cleans the artifacts directory, which contains the generated C headers and libraries
	rm -rf $(artifacts_dir)

# "Private" helper targets

.PHONY: _check-venv
_check-venv:
	@python -c "import sys; exit(1) if sys.prefix == sys.base_prefix else exit(0)" || (echo "Python virtual environment is not activated. Run 'source .venv/bin/activate' to activate it first" && exit 1)

.PHONY: cgo-env
cgo-env: #/ Prints the environment variables needed to build and run the Go language bindings against the engine. Eval the output of this command to set the environment variables.
	@echo "export CGO_LDFLAGS=\"$(CGO_LDFLAGS)\""
	@echo "export LD_LIBRARY_PATH=\"$(LD_LIBRARY_PATH)\"" 