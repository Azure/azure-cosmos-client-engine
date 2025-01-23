# We don't really use the dependency management in this Makefile much because we have fairly complicated cross-language dependencies.
# We just hope that most of the tools we invoke can do incremental compilations.

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

.PHONY: all
all: engine test

.PHONY: engine
engine: engine_c engine_python

# Builds the C API for the engine (primarily for Go).
.PHONY: engine_c
engine_c:
	mkdir -p $(artifacts_dir)/lib
	mkdir -p $(artifacts_dir)/include
	ARTIFACTS_DIR=$(artifacts_dir) cargo build --package $(crate_name) --features c_api --profile $(cargo_profile) $(cargo_args)
	cp $(target_dir)/$(compiled_shared_lib_filename) $(artifacts_dir)/lib/$(shared_lib_filename)
	cp $(target_dir)/$(compiled_static_lib_filename) $(artifacts_dir)/lib/$(static_lib_filename)

# Builds the Python API for the engine.
.PHONY: engine_python
engine_python:
	cd "$(root_dir)/python" && maturin develop --profile $(cargo_profile) $(maturin_args)

.PHONY: test
test: test_go test_python

.PHONY: test_go
test_go:
	@echo "Running Go tests..."
	go -C ./go/engine test ./...

.PHONY: test_python
test_python: check-venv
	@echo "Running Python tests..."
	python -m pytest ./python

# Maturin wants to be in the venv, and the venv is also the only place we have pytest installed. So check that the venv is active.
.PHONY: check-venv
check-venv:
	@python -c "import sys; exit(1) if sys.prefix == sys.base_prefix else exit(0)" || (echo "Python virtual environment is not activated. Run 'source .venv/bin/activate' to activate it first" && exit 1)

.PHONY: clean
clean: clean_rust clean_artifacts

clean_rust:
	cargo clean --profile $(cargo_profile)

clean_artifacts:
	rm -rf $(artifacts_root)