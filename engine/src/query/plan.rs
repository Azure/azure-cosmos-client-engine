use std::{borrow::Cow, collections::HashMap};

use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryPlan {
    pub partitioned_query_execution_info_version: usize,
    pub query_info: QueryInfo,
    pub query_ranges: Vec<QueryRange>,
    // TODO: hybridSearchQueryInfo
}

#[derive(Deserialize, Default, PartialEq, Eq)]
pub enum DistinctType {
    #[default]
    None,
    Ordered,
    Unordered,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct QueryInfo {
    pub distinct_type: DistinctType,
    pub top: Option<u64>,
    pub offset: Option<u64>,
    pub limit: Option<u64>,
    pub order_by: Vec<SortOrder>,
    pub order_by_expressions: Vec<String>,
    pub group_by_expressions: Vec<String>,
    pub group_by_aliases: Vec<String>,
    pub aggregates: Vec<String>,
    pub group_by_alias_to_aggregate_type: HashMap<String, String>,
    pub rewritten_query: Cow<'static, str>,
    pub has_select_value: bool,
    pub has_non_streaming_order_by: bool,
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
