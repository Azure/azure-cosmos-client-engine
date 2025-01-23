# Azure Cosmos DB Client Engine

The Azure Cosmos DB Client Engine is a native library, written in Rust, which provides support functionality for Azure Cosmos DB SDKs.
The primary feature it provides is the Query Engine, which handles fanning out cross-partition queries across individual partitions and aggregating the results.

This repo contains two main components:

* The Client Engine itself, which is a Rust project that produces both a standard Rust crate for use in the Azure SDK for Rust, as well as C-compatible shared and static libraries for use in other languages.
* Client Engine Wrappers for multiple languages. These wrappers handle the packaging and language-specific bindings for the Client Engine and export an API that the Azure Cosmos DB SDK can optionally consume to perform more complicated cross-partition queries.

## Setting up your development environment

The preferred development environment for this repository is a Linux environment, which includes Windows Subsystem for Linux (WSL) version 2.
This repo has full support for GitHub Codespaces, and we recommend using it for development, as it will ensure you have an environment that has all the necessary dependencies installed.

### Manual Setup

If you are unable to use the devenv tool, you will need to install the following dependencies:

* Rust 1.80.0 or later
* Go 1.23 or later
* Python 3.13 or later (do not create a virtual environment manually, as the bootstrap script will do this for you)
* [Maturin](https://www.maturin.rs/installation) for building the Python extension module.
* GNU Make (usually available on Linux distributions)

Once you have those dependencies, run `script/bootstrap` to check your dependencies and set up the dev environment.

## Building

While it's possible to build the engine and non-Python bindings without it, we still highly recommend entering the python virtual environment before working in the repo.
You can do that by running `source .venv/bin/activate` from the root of the repository after bootstrapping with `script/bootstrap`.
Alternatively, you can use the [`direnv` tool](https://github.com/direnv/direnv) to automatically activate the virtual environment when you enter the directory.

We use a `Makefile` to simplify the build process. To build the engine and run all tests on language bindings, simply run `make` from the root of the repository.

You can also run other targets individually.
Run `make help` to see a list of available targets.

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
the rights to use your contribution. For details, visit https://cla.opensource.microsoft.com.

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
