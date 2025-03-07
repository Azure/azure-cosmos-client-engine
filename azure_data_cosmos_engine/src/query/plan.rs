use std::collections::HashMap;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "python_conversions", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[serde(rename_all = "camelCase")]
pub struct QueryPlan {
    #[cfg_attr(feature = "python_conversions", pyo3(item("partitionedQueryExecutionInfoVersion")))]
    pub partitioned_query_execution_info_version: usize,
    #[cfg_attr(feature = "python_conversions", pyo3(item("queryInfo")))]
    pub query_info: QueryInfo,
    #[cfg_attr(feature = "python_conversions", pyo3(item("queryRanges")))]
    pub query_ranges: Vec<QueryRange>,
    // TODO: hybridSearchQueryInfo
}

#[derive(Debug, Deserialize, Default, PartialEq, Eq)]
pub enum DistinctType {
    #[default]
    None,
    Ordered,
    Unordered,
}

#[cfg(feature = "python_conversions")]
impl<'a> pyo3::FromPyObject<'a> for DistinctType {
    fn extract_bound(ob: &pyo3::Bound<'a, pyo3::PyAny>) -> pyo3::PyResult<Self> {
        use pyo3::types::PyAnyMethods;
        use pyo3::types::PyStringMethods;
        let ob = ob.downcast::<pyo3::types::PyString>()?;
        match ob.to_str()? {
            "None" => Ok(Self::None),
            "Ordered" => Ok(Self::Ordered),
            "Unordered" => Ok(Self::Unordered),
            _ => Err(pyo3::exceptions::PyValueError::new_err(
                "invalid DistinctType",
            )),
        }
    }
}

#[derive(Debug, Deserialize, Default)]
#[cfg_attr(feature = "python_conversions", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[serde(default)]
#[serde(rename_all = "camelCase")]
pub struct QueryInfo {
    #[cfg_attr(feature = "python_conversions", pyo3(item("distinctType")))]
    pub distinct_type: DistinctType,
    #[cfg_attr(feature = "python_conversions", pyo3(default))]
    pub top: Option<u64>,
    #[cfg_attr(feature = "python_conversions", pyo3(default))]
    pub offset: Option<u64>,
    #[cfg_attr(feature = "python_conversions", pyo3(default))]
    pub limit: Option<u64>,
    #[cfg_attr(feature = "python_conversions", pyo3(item("orderBy"), default))]
    pub order_by: Vec<SortOrder>,
    #[cfg_attr(feature = "python_conversions", pyo3(item("orderByExpressions"), default))]
    pub order_by_expressions: Vec<String>,
    #[cfg_attr(feature = "python_conversions", pyo3(item("groupByExpressions"), default))]
    pub group_by_expressions: Vec<String>,
    #[cfg_attr(feature = "python_conversions", pyo3(item("groupByAliases"), default))]
    pub group_by_aliases: Vec<String>,
    #[cfg_attr(feature = "python_conversions", pyo3(default))]
    pub aggregates: Vec<String>,
    #[cfg_attr(feature = "python_conversions", pyo3(item("groupByAliasToAggregateType"), default))]
    pub group_by_alias_to_aggregate_type: HashMap<String, String>,
    #[cfg_attr(feature = "python_conversions", pyo3(item("rewrittenQuery"), default))]
    pub rewritten_query: String,
    #[cfg_attr(feature = "python_conversions", pyo3(item("hasSelectValue"), default))]
    pub has_select_value: bool,
    #[cfg_attr(feature = "python_conversions", pyo3(item("hasNonStreamingOrderBy"), default))]
    pub has_non_streaming_order_by: bool,
}

#[derive(Debug, Deserialize)]
pub enum SortOrder {
    Ascending,
    Descending,
}

#[cfg(feature = "python_conversions")]
impl<'a> pyo3::FromPyObject<'a> for SortOrder {
    fn extract_bound(ob: &pyo3::Bound<'a, pyo3::PyAny>) -> pyo3::PyResult<Self> {
        use pyo3::types::PyAnyMethods;
        use pyo3::types::PyStringMethods;
        let ob = ob.downcast::<pyo3::types::PyString>()?;
        match ob.to_str()? {
            "Ascending" => Ok(Self::Ascending),
            "Descending" => Ok(Self::Descending),
            _ => Err(pyo3::exceptions::PyValueError::new_err("invalid SortOrder")),
        }
    }
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "python_conversions", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[serde(rename_all = "camelCase")]
pub struct QueryRange {
    pub min: String,
    pub max: String,
    #[cfg_attr(feature = "python_conversions", pyo3(item("isMinInclusive")))]
    pub is_min_inclusive: bool,
    #[cfg_attr(feature = "python_conversions", pyo3(item("isMaxInclusive")))]
    pub is_max_inclusive: bool,
}
