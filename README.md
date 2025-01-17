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

### Manual Setup

If you are unable to use the devenv tool, you will need to install the following dependencies:

* Rust 1.80.0 or later
* Go 1.23 or later

## Building

To build the client engine itself, you can run `cargo build` from the root of the repository.
However, this will only build the client engine's Rust code and will not build the C bindings, nor the `libcosmoscx` library used by other languages.
Use the `build-driver` script, which is available when you're inside the `devenv shell` environment, to build the client engine and the C bindings.

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
