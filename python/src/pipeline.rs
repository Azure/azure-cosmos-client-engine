// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use std::{ops::DerefMut, sync::Mutex};

use azure_data_cosmos_engine::query::{PartitionKeyRange, PipelineResponse, QueryPipeline};
use pyo3::{
    exceptions, pyclass, pymethods,
    types::{PyAnyMethods, PyBytes, PyBytesMethods, PyInt, PyList, PyString, PyStringMethods},
    Bound, Py, PyAny, PyErr, PyResult, Python,
};

#[pyclass(frozen, name = "QueryPipeline")]
pub struct NativeQueryPipeline {
    // Python may access this object on any thread.
    pipeline: Mutex<QueryPipeline>,
}

// All methods in this block are NOT python-accessible, and only visible to Rust code
impl NativeQueryPipeline {
    #[inline(always)]
    fn pipeline(&self) -> PyResult<impl DerefMut<Target = QueryPipeline> + '_> {
        self.pipeline
            .lock()
            .map_err(|_| PyErr::new::<exceptions::PyRuntimeError, _>("lock poisoned"))
    }
}

// All methods in this block are python-accessible
#[pymethods]
impl NativeQueryPipeline {
    #[new]
    fn new(query: Bound<PyString>, plan: Bound<PyAny>, pkranges: Bound<PyAny>) -> PyResult<Self> {
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

    fn next_batch(&self, py: Python) -> PyResult<Option<PyPipelineResult>> {
        let mut pipeline = self.pipeline()?;
        let result = pipeline.run()?;
        Ok(Some(PyPipelineResult::new(py, result)?))
    }

    fn provide_data<'py>(
        &self,
        pkrange_id: Bound<'py, PyString>,
        request_id: Bound<'py, PyInt>,
        data: Bound<'py, PyBytes>,
        continuation: Option<Bound<'py, PyString>>,
    ) -> PyResult<()> {
        let mut pipeline = self.pipeline()?;
        let pkrange_id = pkrange_id.to_str()?;
        let request_id = request_id.extract()?;
        let continuation = continuation
            .map(|s| s.to_str().map(|s| s.to_string()))
            .transpose()?;
        // Pass the raw bytes directly to the pipeline
        pipeline.provide_data(pkrange_id, request_id, data.as_bytes(), continuation)?;
        Ok(())
    }
}

#[pyclass(name = "PipelineResult")]
pub struct PyPipelineResult {
    #[pyo3(get)]
    items: Py<PyList>,
    #[pyo3(get)]
    requests: Py<PyList>,
    #[pyo3(get)]
    terminated: bool,
}

impl PyPipelineResult {
    pub fn new(py: Python, result: PipelineResponse) -> PyResult<Self> {
        let items = result
            .items
            .into_iter()
            .map(|r| PyBytes::new(py, r.get().as_bytes()));
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

#[pyclass(name = "DataRequest")]
pub struct PyDataRequest {
    #[pyo3(get)]
    pub pkrange_id: Py<PyString>,
    #[pyo3(get)]
    pub continuation: Option<Py<PyString>>,
}
