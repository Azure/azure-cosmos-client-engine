// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use std::borrow::Cow;

use serde::Deserialize;

mod aggregators;
pub mod node;
mod pipeline;
mod plan;
mod producer;
mod query_result;

#[cfg(feature = "query_engine")]
mod engine;

#[cfg(feature = "query_engine")]
pub use engine::*;

pub use pipeline::{QueryPipeline, SupportedFeatures, SUPPORTED_FEATURES};
pub use plan::{DistinctType, QueryInfo, QueryPlan, QueryRange, SortOrder};
pub use query_result::{QueryClauseItem, QueryResult, QueryResultShape};

/// Features that may be required by the Query Engine.
///
/// The query pipeline provides the language bindings a list of features that it can support, using these values.
/// The language binding can then forward that information to the gateway when generating a query plan, which allows the gateway to reject queries that the engine cannot support.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueryFeature {
    None,
    Aggregate,
    CompositeAggregate,
    Distinct,
    GroupBy,
    MultipleAggregates,
    MultipleOrderBy,
    OffsetAndLimit,
    OrderBy,
    Top,
    NonValueAggregate,
    DCount,
    NonStreamingOrderBy,
    ListAndSetAggregate,
    CountIf,
    HybridSearch,
    WeightedRankFusion,
    HybridSearchSkipOrderByRewrite,
}

#[derive(Debug, Clone)]
pub struct Query {
    /// The text of the query.
    pub text: String,

    /// The parameters of the query, pre-encoded as a JSON object suitable to being the `parameters` field of a Cosmos query.
    pub encoded_parameters: Option<Box<serde_json::value::RawValue>>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(
    feature = "python_conversions",
    derive(pyo3::FromPyObject),
    pyo3(from_item_all)
)]
#[serde(rename_all = "camelCase")]
pub struct PartitionKeyRange {
    id: String,
    #[cfg_attr(feature = "python_conversions", pyo3(item("minInclusive")))]
    min_inclusive: String,
    #[allow(dead_code)]
    #[cfg_attr(feature = "python_conversions", pyo3(item("maxExclusive")))]
    max_exclusive: String,
}

impl PartitionKeyRange {
    pub fn new(
        id: impl Into<String>,
        min_inclusive: impl Into<String>,
        max_exclusive: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            min_inclusive: min_inclusive.into(),
            max_exclusive: max_exclusive.into(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
// TODO: pk values here are all currently strings - we need the same sort of PartitionKeyValue
// logic used in the main Rust SDK in order to compare and be able to use it within this method.
pub struct ItemIdentity {
    #[serde(rename = "PartitionKeyValue")]
    partition_key_value: String,
    #[serde(rename = "ID")]
    id: String,
}

impl ItemIdentity {
    pub fn new(id: impl Into<String>, partition_key_value: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            partition_key_value: partition_key_value.into(),
        }
    }
}

#[derive(Debug)]
pub struct QueryChunk {
    pub pk_range_id: String,
    pub items: Vec<QueryChunkItem>
}

#[derive(Debug)]
pub struct QueryChunkItem {
    pub index: usize,
    pub id: String,
    pub partition_key_value: String,
}

/// Describes a request for additional data from the pipeline.
///
/// This value is returned when the pipeline needs more data to continue processing.
/// It contains the information necessary for the caller to make an HTTP request to the Cosmos APIs to fetch the next batch of data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "python_conversions", derive(pyo3::IntoPyObject))]
pub struct DataRequest {
    /// A unique identifier for this request that can be used to match it with it's response.
    pub id: u64,
    pub pkrange_id: Cow<'static, str>,
    pub continuation: Option<String>,
    pub query: Option<String>,
    pub include_parameters: bool,
}

impl DataRequest {
    pub fn new(
        id: u64,
        pkrange_id: impl Into<Cow<'static, str>>,
        continuation: Option<String>,
        query: Option<String>,
    ) -> Self {
        Self {
            id,
            pkrange_id: pkrange_id.into(),
            continuation,
            query: query,
            include_parameters: true,
        }
    }

    pub fn with_query(
        id: u64,
        pkrange_id: impl Into<Cow<'static, str>>,
        continuation: Option<String>,
        query: impl Into<String>,
        include_parameters: bool,
    ) -> Self {
        Self {
            id,
            pkrange_id: pkrange_id.into(),
            continuation,
            query: Some(query.into()),
            include_parameters,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PipelineResponse {
    /// The items returned by the pipeline.
    pub items: Vec<Box<serde_json::value::RawValue>>,

    /// Requests for additional data from the pipeline.
    ///
    /// If [`PipelineResponse::terminated`] is `true`, this will be empty and can be ignored.
    pub requests: Vec<DataRequest>,

    /// Indicates if the pipeline has terminated.
    ///
    /// If this is true, no further items will be produced, even if more data is provided.
    pub terminated: bool,
}

impl PipelineResponse {
    pub const TERMINATED: Self = Self {
        items: Vec::new(),
        requests: Vec::new(),
        terminated: true,
    };
}
