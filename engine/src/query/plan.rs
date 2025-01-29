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
    distinct_type: String,
    order_by: Vec<SortOrder>,
    order_by_expressions: Vec<String>,
    rewritten_query: String,
}

#[derive(Deserialize)]
pub enum SortOrder {
    Ascending,
    Descending,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryRange {
    pub min: String,
    pub max: String,
    pub is_min_inclusive: bool,
    pub is_max_inclusive: bool,
}
