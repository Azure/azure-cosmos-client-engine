use std::borrow::Cow;

use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryPlan {
    pub partitioned_query_execution_info_version: usize,
    pub query_info: QueryInfo,
    pub query_ranges: Vec<QueryRange>,
    // TODO: hybridSearchQueryInfo
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryInfo {
    pub distinct_type: Cow<'static, str>,
    pub order_by: Vec<SortOrder>,
    pub order_by_expressions: Vec<String>,
    pub rewritten_query: Cow<'static, str>,
}

#[derive(Deserialize)]
pub enum SortOrder {
    Ascending,
    Descending,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryRange {
    pub min: Cow<'static, str>,
    pub max: Cow<'static, str>,
    pub is_min_inclusive: bool,
    pub is_max_inclusive: bool,
}
