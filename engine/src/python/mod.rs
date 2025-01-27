#[pyo3::pymodule]
mod _azure_cosmoscx {
    use pyo3::prelude::*;

    #[pyfunction]
    fn version() -> &'static str {
        env!("CARGO_PKG_VERSION")
    }
}
