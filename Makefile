# We don't really use the dependency management in this Makefile much because we have fairly complicated cross-language dependencies.
# We just hope that most of the tools we invoke can do incremental compilations.

# NOTE: Any line that starts '#/' will appear in the help output when running 'make help'.
# In addition, any '#/' comment that appears after a target will be used to describe that target in the help output.

#/ Makefile for the Azure Data Cosmos Client Engine

root_dir := $(shell git rev-parse --show-toplevel)
configuration ?= debug
target_root ?= $(root_dir)/target
artifacts_root ?= $(root_dir)/artifacts
target_dir := $(target_root)/$(configuration)
artifacts_dir := $(artifacts_root)/$(configuration)
crate_name := azure_data_cosmos_client_engine
shared_lib_name := cosmoscx
header_name := $(shared_lib_name).h

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
	compiled_shared_lib_filename := $(crate_name).dll
	compiled_static_lib_filename := $(crate_name).a
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


# Cargo calls the 'debug' configuration 'dev', yet it still builds to a 'debug' directory in the target directory.
ifeq ($(configuration),debug)
	cargo_profile = dev
else
	cargo_profile = $(configuration)
endif

# Default target, don't put any targets above this one.
.PHONY: all
all: engine test #/ Builds the engine and runs all tests

.PHONY: help
help: #/ Show this help
	@egrep -h '^#/\s' $(MAKEFILE_LIST) | sed -e 's/^#\/\s*//'
	@echo ""
	@echo "Targets:"
	@egrep -h '\s#/\s' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?#/ *"}; {printf "\033[36m%-30s\033[0m %s\n", $$1, $$2}'

.PHONY: engine
engine: engine_c engine_python #/ Builds all versions of the engine

.PHONY: engine_c
engine_c: #/ Builds the C API for the engine, producing the shared and static libraries
	mkdir -p $(artifacts_dir)/lib
	mkdir -p $(artifacts_dir)/include
	COSMOSCX_INCLUDE_DIR=$(artifacts_dir)/include cargo build --package $(crate_name) --features c_api --profile $(cargo_profile) $(cargo_args)
	cp $(target_dir)/$(compiled_shared_lib_filename) $(artifacts_dir)/lib/$(shared_lib_filename)
	cp $(target_dir)/$(compiled_static_lib_filename) $(artifacts_dir)/lib/$(static_lib_filename)

.PHONY: engine_python
engine_python: #/ Builds the python extension module for the engine
	cd "$(root_dir)/python" && maturin develop --profile $(cargo_profile) $(maturin_args)

.PHONY: test
test: test_go test_python #/ Runs all language binding tests

.PHONY: test_go
test_go: #/ Runs the Go language binding tests
	@echo "Running Go tests..."
	go -C ./go/engine test ./...

.PHONY: test_python
test_python: _check-venv #/ Runs the Python language binding tests
	@echo "Running Python tests..."
	python -m pytest ./python

.PHONY: clean
clean: clean_rust clean_artifacts #/ Cleans all build artifacts

.PHONY: clean_rust
clean_rust: #/ Cleans all Rust build artifacts
	cargo clean --profile $(cargo_profile)

.PHONY: clean_artifacts
clean_artifacts: #/ Cleans the artifacts directory, which contains the generated C headers and libraries
	rm -rf $(artifacts_root)

# "Private" helper targets

.PHONY: _check-venv
_check-venv:
	@python -c "import sys; exit(1) if sys.prefix == sys.base_prefix else exit(0)" || (echo "Python virtual environment is not activated. Run 'source .venv/bin/activate' to activate it first" && exit 1)