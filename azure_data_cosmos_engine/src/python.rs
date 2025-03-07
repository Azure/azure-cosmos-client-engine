//! Contains the definition of the Python Extension Module for the Rust Client Engine
//!
//! The [`native`] module is exported to Python as `_azure_cosmoscx`.

/// Python
#[pyo3::pymodule(name = "_azure_cosmoscx")]
mod native {
    use std::{error::Error, ops::DerefMut, sync::Mutex};

    use pyo3::{
        sync::GILOnceCell,
        types::{
            PyAnyMethods, PyBool, PyDict, PyList, PyListMethods, PyString, PyStringMethods, PyType,
        },
        Bound, FromPyObject, Py, PyAny, PyErr, PyResult, Python,
    };
    use tracing_subscriber::EnvFilter;

    use crate::{
        query::{PartitionKeyRange, PipelineResponse, QueryClauseItem, QueryPipeline, QueryResult},
        ErrorKind,
    };

    /// Lazy-initialized static that holds the "numbers.Number" Python type, which we'll need when comparing values.
    static NUMBERS_DOT_NUMBER: GILOnceCell<Py<PyType>> = GILOnceCell::new();

    #[pyo3::pyfunction]
    fn version() -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    #[pyo3::pyfunction]
    fn enable_tracing() {
        // TODO: We could probably wrap Python's OpenTracing API here.
        tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::from_env("COSMOSCX_LOG"))
            .init();
    }

    #[pyo3::pyclass(frozen, name = "QueryPipeline")]
    struct NativeQueryPipeline {
        // Python may access this object on any thread.
        pipeline: Mutex<QueryPipeline<Py<PyAny>, PyQueryClauseItem>>,
    }

    // All methods in this block are NOT python-accessible, and only visible to Rust code
    impl NativeQueryPipeline {
        #[inline(always)]
        fn pipeline<'a>(
            &'a self,
        ) -> PyResult<impl DerefMut<Target = QueryPipeline<Py<PyAny>, PyQueryClauseItem>> + 'a>
        {
            self.pipeline
                .lock()
                .map_err(|_| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>("lock poisoned"))
        }
    }

    // All methods in this block are python-accessible
    #[pyo3::pymethods]
    impl NativeQueryPipeline {
        #[new]
        fn new(
            query: Bound<PyString>,
            plan: Bound<PyAny>,
            pkranges: Bound<PyAny>,
        ) -> PyResult<Self> {
            let query = query.to_str()?;
            let plan = plan.extract()?;
            let pkranges: Vec<PartitionKeyRange> = pkranges.extract()?;
            let pipeline = QueryPipeline::new(query, plan, pkranges)?;

            Ok(Self {
                pipeline: Mutex::new(pipeline),
            })
        }

        fn query<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyString>> {
            let pipeline = self.pipeline()?;
            Ok(PyString::new(py, pipeline.query()))
        }

        fn next_batch<'py>(&self, py: Python<'py>) -> PyResult<Option<PyPipelineResult>> {
            let mut pipeline = self.pipeline()?;
            let result = pipeline.run()?;
            Ok(Some(PyPipelineResult::new(py, result)?))
        }

        fn provide_data<'py>(
            &self,
            pkrange_id: Bound<'py, PyString>,
            data: Bound<'py, PyList>,
            continuation: Option<Bound<'py, PyString>>,
        ) -> PyResult<()> {
            let mut pipeline = self.pipeline()?;
            let pkrange_id = pkrange_id.to_str()?;
            let continuation = continuation
                .map(|s| s.to_str().map(|s| s.to_string()))
                .transpose()?;
            let data: Vec<QueryResult<Py<PyAny>, PyQueryClauseItem>> =
                if pipeline.results_are_bare_payloads() {
                    data.iter()
                        .map(|a| QueryResult::from_payload(a.unbind()))
                        .collect()
                } else {
                    data.iter()
                        .map(|a| a.extract())
                        .collect::<Result<Vec<_>, _>>()?
                };
            pipeline.provide_data(pkrange_id, data, continuation)?;
            Ok(())
        }
    }

    #[pyo3::pyclass(name = "PipelineResult")]
    struct PyPipelineResult {
        #[pyo3(get)]
        items: Py<PyList>,
        #[pyo3(get)]
        requests: Py<PyList>,
        #[pyo3(get)]
        terminated: bool,
    }

    impl PyPipelineResult {
        pub fn new<'py>(py: Python<'py>, result: PipelineResponse<Py<PyAny>>) -> PyResult<Self> {
            let items = result.items.into_iter().map(|item| item);
            let requests = result.requests.into_iter().map(|r| PyDataRequest {
                pkrange_id: PyString::new(py, r.pkrange_id.as_ref()).unbind(),
                continuation: r.continuation.map(|s| PyString::new(py, &s).unbind()),
            });
            let items = PyList::new(py, items)?.unbind();
            let requests = PyList::new(py, requests)?.unbind();
            Ok(Self {
                items,
                requests,
                terminated: result.terminated,
            })
        }
    }

    #[pyo3::pyclass(name = "DataRequest")]
    struct PyDataRequest {
        #[pyo3(get)]
        pub pkrange_id: Py<PyString>,
        #[pyo3(get)]
        pub continuation: Option<Py<PyString>>,
    }

    #[derive(Debug, FromPyObject)]
    #[pyo3(transparent)]
    struct PyQueryClauseItem(Py<PyDict>);

    impl QueryClauseItem for PyQueryClauseItem {
        fn compare(&self, other: &Self) -> crate::Result<std::cmp::Ordering> {
            Python::with_gil(|py| {
                let left = self.0.bind(py);
                let right = other.0.bind(py);

                let (left_ordinal, left_item) = type_ordinal_for_any(py, left)?;
                let (right_ordinal, right_item) = type_ordinal_for_any(py, right)?;

                if left_ordinal != right_ordinal {
                    return Ok(left_ordinal.cmp(&right_ordinal));
                }

                match (left_item, right_item) {
                    (None, None) => return Ok(std::cmp::Ordering::Equal),
                    (Some(l), Some(r)) => return Ok(l.compare(r)?),

                    // These should be the same type if we got here.
                    _ => unreachable!(),
                }
            })
        }
    }

    fn type_ordinal_for_any<'py>(
        py: Python<'py>,
        value: &Bound<'py, PyAny>,
    ) -> crate::Result<(usize, Option<Bound<'py, PyAny>>)> {
        // Based on sdk/cosmos/azure-cosmos/azure/cosmos/_execution_context/document_producer.py
        // From the Python SDK

        // if "item" not in value:
        if !value.contains("item")? {
            return Ok((0, None));
        }

        let val = value.get_item("item")?;

        // if val is None:
        if val.is_none() {
            return Ok((1, Some(val)));
        }

        // if isinstance(val, bool):
        if val.is_instance_of::<PyBool>() {
            return Ok((2, Some(val)));
        }

        // if isinstance(val, numbers.Number):
        let numbers_dot_number = NUMBERS_DOT_NUMBER.import(py, "numbers", "Number")?;
        if val.is_instance(numbers_dot_number)? {
            return Ok((4, Some(val)));
        }

        if val.is_instance_of::<PyString>() {
            return Ok((5, Some(val)));
        }

        Err(
            PyErr::new::<pyo3::exceptions::PyTypeError, _>(format!("unknown type: {}", val.str()?))
                .into(),
        )
    }

    impl From<PyErr> for crate::Error {
        fn from(err: PyErr) -> Self {
            ErrorKind::PythonError.with_source(err)
        }
    }

    impl From<crate::Error> for PyErr {
        fn from(err: crate::Error) -> Self {
            if err.kind() == ErrorKind::PythonError {
                if err.source().is_none() {
                    return PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(err.to_string());
                }
                let err = err.into_source().expect("we just checked that it was Some");
                let err = err
                    .downcast::<PyErr>()
                    .expect("PythonError's source must be a PyErr");
                *err
            } else {
                PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(err.to_string())
            }
        }
    }
}
