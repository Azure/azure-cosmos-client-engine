// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use azure_data_cosmos_engine::query::QueryClauseItem;
use pyo3::{
    exceptions,
    sync::GILOnceCell,
    types::{PyAnyMethods, PyBool, PyDict, PyString, PyType},
    Bound, FromPyObject, Py, PyAny, PyErr, Python,
};

/// Lazy-initialized static that holds the "numbers.Number" Python type, which we'll need when comparing values.
static NUMBERS_DOT_NUMBER: GILOnceCell<Py<PyType>> = GILOnceCell::new();

#[derive(Debug, FromPyObject)]
#[pyo3(transparent)]
pub struct PyQueryClauseItem(Py<PyDict>);

impl QueryClauseItem for PyQueryClauseItem {
    fn compare(&self, other: &Self) -> Result<std::cmp::Ordering, azure_data_cosmos_engine::Error> {
        Python::with_gil(|py| {
            let left = self.0.bind(py);
            let right = other.0.bind(py);

            let (left_ordinal, left_item) = type_ordinal_for_any(py, left)?;
            let (right_ordinal, right_item) = type_ordinal_for_any(py, right)?;

            if left_ordinal != right_ordinal {
                return Ok(left_ordinal.cmp(&right_ordinal));
            }

            match (left_item, right_item) {
                (None, None) => Ok(std::cmp::Ordering::Equal),
                (Some(l), Some(r)) => Ok(l.compare(r)?),

                // These should be the same type if we got here.
                _ => unreachable!(),
            }
        })
    }
}

fn type_ordinal_for_any<'py>(
    py: Python<'py>,
    value: &Bound<'py, PyAny>,
) -> Result<(usize, Option<Bound<'py, PyAny>>), azure_data_cosmos_engine::Error> {
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

    Err(PyErr::new::<exceptions::PyTypeError, _>(format!("unknown type: {}", val.str()?)).into())
}
