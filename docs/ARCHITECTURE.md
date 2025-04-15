# Client Engine Architecture

The Cosmos Client Engine is a native library that can be referenced from any other Cosmos DB client SDK (such as the .NET, Java, Python, JS, Go, and Rust SDKs).
It implements complex and common logic that is shared across all SDKs, such as:

* Cross-partition query aggregation
* Thin client protocol and transport
* Any other common logic that is shared across all SDKs and appropriate to be implemented in a native library

## Goals

The goals of the Cosmos Client Engine are:

* To provide single well-tested implementations of common client SDK logic.
* To accelerate development of language-specific SDKs by reducing the logic that needs to be reimplemented across each SDK.

Notable non-goals include:

* To provide performance benefits above and beyond what can be achieved in language-specific SDKs. We believe the engine can provide competitive performance that meets our needs, but we are not using a native library to avoid properly optimizing the language-specific SDKs. Native interop often carries a performance overhead, and while we will minimize this, we realize that if performance was our only goal, implementing the logic in the language-specific SDKs may be the better approach.

* To obsolete the full language-specific SDKs in favor of wrapping a native library SDK. The client engine is not a full SDK, and it is not intended to be used as a standalone SDK. It is a library that is intended to be referenced from other SDKs. Language-specific wrappers for SDKs exist in other platforms and often come with significant tradeoffs (such as poor integration with language-specific observability and diagnostics, poor performance, deployment complexity, etc.), which we don't want to impose on all SDK users. Some of those tradeoffs will apply when working with the client engine, but our goal is to minimize them as much as possible.

## Deployment and Artifacts

Before talking about architecture, it's useful to understand how the Cosmos Client Engine is packaged: What artifacts are built here, and how are they distributed?

### `azure_data_cosmos_engine`

The `azure_data_cosmos_engine` crate, found in the `azure_data_cosmos_engine` directory, is a Rust library (crate) that provides the core functionality of the Cosmos Client Engine.
It is a pure Rust library that produces an artifact that can only be used from Rust code, using a Rust API.
All the actual logic of the Cosmos Client Engine can be found in this crate, so if you're planning to implement a new query pipeline operator, for example, this is where to look.

### `cosmoscx` C-compatible Library

The `cosmoscx` project, found in the `cosmoscx` directory, is a Rust project that produces a C-compatible static and shared library that can be used from C/C++ code, or from any other language that can interop with C-compatible libraries.
As a Rust project, it can reference the `azure_data_cosmos_engine` crate, wrap it, and expose a C-compatible API that can be used from other languages.
We use the `cbindgen` tool to generate a C header file, `include/cosmoscx.h`, that describes the C-compatible API in C.
This header can be referenced from a C/C++ application, or used with other language-specific tools that can import C header files (such as Cgo).
This library doesn't really contain any logic of its own, aside from some lifetime management and error handling code.

### `azcosmoscx` Go package

The `azcosmoscx` Go package, found in the `go/azcosmoscx` directory, is a Go package that wraps the `cosmoscx` C-compatible library.
If you consider `cosmoscx` the "Rust-to-C" wrapper, this is the "C-to-Go" wrapper that results in a Go package that can call Rust code.
This package uses Cgo to interoperate with the `cosmoscx` library, and exports a Go API that implements the (currently-unstable) `QueryEngine` interface provided by the standard Azure Cosmos Go SDK.

### `azure_cosmoscx` Python package

The `azure_cosmoscx` Python package, found in the `python` directory, is a Python native extension, written in Rust using [maturin](https://maturin.rs/) and [PyO3](https://pyo3.rs/), that wraps the `azure_data_cosmos_engine` crate
and provides a Python API that can be used from Python code.
This is mostly a proof-of-concept, and is not currently funded or supported, but provides an example of a second integration between a language-specific SDK and the Cosmos Client Engine.

## Query Pipeline

The Query Pipeline is one of the features provided by the Cosmos Client Engine.
It provides a mechanism for managing cross-partition query execution and result aggregation.
The Query Pipeline is implemented in the `query` module of the `azure_data_cosmos_engine` crate.
The API itself is well-documented in the code, and can be viewed using `script/docs-server`, but at a high-level, the pipeline exposes the following APIs:

1. The SDK creates a `Pipeline` object, representing a single query pipeline, for a specific query plan and set of partition key ranges.
2. The SDK calls `Pipeline::run()`, which will produce any query results as well as a list of additional single-partition queries that need to be executed.
3. The SDK yields those items to the user, and then makes the single-partition queries requested by the pipeline.
4. Once the SDK has the results of the single-partition queries back, it can call `Pipeline::provide_data()` to provide the results back to the pipeline.
5. Then, the SDK loops back around to 2 and calls `Pipeline::run()` again to get the next set of results and requests.