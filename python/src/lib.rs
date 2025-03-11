//! Contains the definition of the Python Extension Module for the Rust Client Engine

use pyo3::{
    pyfunction, pymodule,
    types::{PyModule, PyModuleMethods},
    wrap_pyfunction, Bound, PyResult,
};
use tracing_subscriber::EnvFilter;

mod pipeline;
mod query_clause;

#[pymodule(name = "_azure_cosmoscx")]
fn azure_cosmoscx(m: &Bound<PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(version, m)?)?;
    m.add_function(wrap_pyfunction!(enable_tracing, m)?)?;
    m.add_class::<pipeline::NativeQueryPipeline>()?;
    m.add_class::<pipeline::PyPipelineResult>()?;
    m.add_class::<pipeline::PyDataRequest>()?;
    Ok(())
}

#[pyfunction]
fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[pyfunction]
fn enable_tracing() {
    // TODO: We could probably wrap Python's OpenTracing API here.

    // Ignore failure to init, it just means tracing is already enabled.
    _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_env("COSMOSCX_LOG"))
        .try_init();
}
