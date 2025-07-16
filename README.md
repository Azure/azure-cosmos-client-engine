# Azure Cosmos DB Client Engine

**IMPORTANT: This project is currently experimental and not officially supported for production workloads at this time.**

The Azure Cosmos DB Client Engine is a native library, written in Rust, which provides support functionality for Azure Cosmos DB SDKs.
The primary feature it provides is the Query Engine, which handles fanning out cross-partition queries across individual partitions and aggregating the results.

This repo contains several components:

* `azure_data_cosmos_engine` - A Rust library that implements the core functionality of the engine and exports a pure Rust API.
* `baselines` - A set of query correctness data, and a .NET application to generate that data.
* `cosmoscx` - A Rust `cdylib`/`staticlib` that exposes a C API, `libcosmoscx`, for the engine, which can be used by other languages.
* `python` - A Python module, using [PyO3](https://pyo3.rs) and built with [Maturin](https://maturin.rs), that wraps the Rust engine and provides a Pythonic interface to the engine.
* `go/azcosmoscx` - A Go module that wraps `libcosmoscx` and provides a Go interface to the engine.
* `include` - A C header file, `cosmoscx.h`, that defines the C API for the engine. This is generated from the Rust code using [cbindgen](https://github.com/mozilla/cbindgen)

## Supported Platforms

The engine itself is intended to be largely cross-platform and work on any platform that Rust supports.
However, we only test and produce builds for the following platforms:

* `x86_64-unknown-linux-gnu` - Linux (GNU libc) x86_64
* `aarch64-apple-darwin` - macOS ARM64
* `x86_64-pc-windows-msvc` - Windows (MSVC ABI) x86_64.
  * **NOTE**: We do not currently support the Go bindings on this platform
* `x86_64-pc-windows-gnu` - Windows (GNU ABI and libc) x86_64.

## Setting up your development environment

The preferred development environment for this repository is a Linux environment, which includes Windows Subsystem for Linux (WSL) version 2.
This repo has full support for GitHub Codespaces and VS Code Dev Containers, and we recommend using those for development, as they will ensure you have an environment that has all the necessary dependencies installed.

### Manual Setup

If you are unable to use a Codespace or Dev Container, you will need to install the following dependencies:

* Rust 1.80.0 or later
* Go 1.23 or later
* Python 3.13 or later (do not create a virtual environment manually, as the bootstrap script will do this for you)
* GNU Make (usually available on Linux distributions)

Once you have those dependencies, run `script/bootstrap` to check your dependencies, install additional dependencies (such as [Maturin](https://maturin.rs), for building Python modules), and set up the dev environment.

## Reviewing API docs

A lot of our documentation can be found within the Rust code itself, using doc comments (`///`).
To view this documentation, you can either:

1. (Recommended on all environments) Run the `script/docs-server` script to start a process that will watch the Rust code, regenerate docs whenever it changes, and serve them on `localhost:8000`. This is the recommended way to view the docs, as it will automatically update as you make changes to the code.
2. (Only on local machines) Run `cargo doc` to generate docs locally and open them in your browser.

## Building

While it's possible to build the engine and non-Python bindings without it, we still highly recommend entering the python virtual environment before working in the repo.
You can do that by running `source .venv/bin/activate` from the root of the repository after bootstrapping with `script/bootstrap`.
Alternatively, you can use the [`direnv` tool](https://github.com/direnv/direnv) to automatically activate the virtual environment when you enter the directory.

We use a `Makefile` to simplify the build process. To build the engine and run all tests on language bindings, simply run `make` from the root of the repository.

You can also run other targets individually.
Run `make help` to see a list of available targets.

### Working on the client engine

The client engine is located in `engine/` and is essentially a standard Rust project.
You can build the engine by running `make engine` from the root of the repository.

If you change the C API, found in the `c_api` module, you will need to regenerate the C header file.
Because this header file is used to build the Go engine, it's committed to the repository and must be updated manually and committed whenever you make a change to the C API.
The CI will ensure you've done this correctly and fail the build if the headers don't match the Rust library.
Don't update the header file yourself, use `make headers` to run `cbindgen` to generate the header file.

### Building and testing Go

**After** running `make engine`, you can test the Go bindings by running `go -C ./go/engine test ./...`.
If you haven't run `make engine` yet, the Go tests will fail to compile with an error like this, indicating the `artifacts` directory isn't properly set up:

```
> go -C ./go/engine test ./...
?       github.com/Azure/azure-cosmos-client-engine/go/engine   [no test files]
# github.com/Azure/azure-cosmos-client-engine/go/engine/native
native/native.go:9:11: fatal error: 'cosmoscx.h' file not found
    9 |  #include <cosmoscx.h>
      |           ^~~~~~~~~~~~
1 error generated.
FAIL    github.com/Azure/azure-cosmos-client-engine/go/engine/native [build failed]
FAIL
```

### Building and testing Python

> [!NOTE]
> For now, the Python module we create is named `azure_cosmoscx`.
> At some point before release, we may rearrange this so that we create a module named `azure.cosmos.client_engine` instead.
> However, this requires changes to the `azure.cosmos` package (to support being a namespace module), which is in the Azure SDK for Python repository.

**After** running `make engine`, you can test the Python bindings by running `make test_python`
If you haven't run `make engine` yet, the Python tests will fail to compile with an error like this, indicating the Python venv isn't properly set up:

```
ImportError while importing test module '/home/ashleyst/code/Azure/azure-cosmos-client-engine/python/test/test_engine_version.py'.
Hint: make sure your test modules/packages have valid Python names.
Traceback:
/nix/store/kln911id1b6cxcpflzm263s58wa3d7wg-python3-3.12.7-env/lib/python3.12/importlib/__init__.py:90: in import_module
    return _bootstrap._gcd_import(name[level:], package, level)
python/test/test_engine_version.py:2: in <module>
    import azure_cosmoscx
E   ModuleNotFoundError: No module named 'azure_cosmoscx'
```

## Contributing

This project welcomes contributions and suggestions.  Most contributions require you to agree to a
Contributor License Agreement (CLA) declaring that you have the right to, and actually do, grant us
the rights to use your contribution. For details, visit <https://cla.opensource.microsoft.com>.

When you submit a pull request, a CLA bot will automatically determine whether you need to provide
a CLA and decorate the PR appropriately (e.g., status check, comment). Simply follow the instructions
provided by the bot. You will only need to do this once across all repos using our CLA.

This project has adopted the [Microsoft Open Source Code of Conduct](https://opensource.microsoft.com/codeofconduct/).
For more information see the [Code of Conduct FAQ](https://opensource.microsoft.com/codeofconduct/faq/) or
contact [opencode@microsoft.com](mailto:opencode@microsoft.com) with any additional questions or comments.

## Trademarks

This project may contain trademarks or logos for projects, products, or services. Authorized use of Microsoft
trademarks or logos is subject to and must follow
[Microsoft's Trademark & Brand Guidelines](https://www.microsoft.com/en-us/legal/intellectualproperty/trademarks/usage/general).
Use of Microsoft trademarks or logos in modified versions of this project must not cause confusion or imply Microsoft sponsorship.
Any use of third-party trademarks or logos are subject to those third-party's policies.
