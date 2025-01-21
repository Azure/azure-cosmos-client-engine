# Azure Cosmos DB Client Engine

The Azure Cosmos DB Client Engine is a native library, written in Rust, which provides support functionality for Azure Cosmos DB SDKs.
The primary feature it provides is the Query Engine, which handles fanning out cross-partition queries across individual partitions and aggregating the results.

This repo contains two main components:

* The Client Engine itself, which is a Rust project that produces both a standard Rust crate for use in the Azure SDK for Rust, as well as C-compatible shared and static libraries for use in other languages.
* Client Engine Wrappers for multiple languages. These wrappers handle the packaging and language-specific bindings for the Client Engine and export an API that the Azure Cosmos DB SDK can optionally consume to perform more complicated cross-partition queries.

## Setting up your development environment

The preferred development environment for this repository is a Linux environment, which includes Windows Subsystem for Linux (WSL) version 2.
The ideal way to configure your environment is to install the [devenv](https://devenv.sh) tool in your Linux environment.
Once you've installed this tool in your environement, you can run `devenv shell` in the root of the repository to prepare an isolated development environment with all the necessary dependencies.
Alternatively, configure your shell with devenv's [Automatic Shell Activation](https://devenv.sh/automatic-shell-activation/) feature and the development environment will be automatically configured when you change directory in to the repository.

The `devenv` tool also provides instructions on how to use VS Code with your development environment: https://devenv.sh/editor-support/vscode/.
Once you set that up, Rust, Python, and Go integration should mostly Just Work™.

### Manual Setup

If you are unable to use the devenv tool, you will need to install the following dependencies:

* Rust 1.80.0 or later
* Go 1.23 or later
* Python 3. Using a [virtualenv](https://docs.python.org/3/library/venv.html) is also HIGHLY recommended.

## Building

To build the client engine itself, you can run `cargo build` from the root of the repository.
However, this will only build the client engine's Rust code and will not build the C bindings, nor the `libcosmoscx` library used by other languages.
Use the `build-driver` script, which is available when you're inside the `devenv shell` environment, to build the client engine and the C bindings.
This will produce the necessary native libraries and C include files in the `artifacts` directory in the root of the repository.
It will also compile the Python native module and install it into the virtualenv created by the `devenv` tool.

### Building and testing Go

**After** running `build-driver`, you can test the Go bindings by running `go -C ./go/engine test ./...`.
If you haven't run `build-driver` yet, the Go tests will fail to compile with an error like this, indicating the `artifacts` directory isn't properly set up:

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
At some point before release, we may rearrange this so that we create a module named `azure.cosmos.client_engine` instead.
However, this requires changes to the `azure.cosmos` package (to support being a namespace module), which is in the Azure SDK for Python repository.

**After** running `build-driver`, you can test the Python bindings by running `python -m pytest ./python`.
If you haven't run `build-driver` yet, the Python tests will fail to compile with an error like this, indicating the Python venv isn't properly set up:

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
