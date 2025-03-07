use std::borrow::Cow;

use serde::Deserialize;

pub mod node;
mod pipeline;
mod plan;
mod producer;
mod query_result;

pub use pipeline::{QueryPipeline, SUPPORTED_FEATURES};
pub use plan::{DistinctType, QueryInfo, QueryPlan, QueryRange, SortOrder};
pub use query_result::{JsonQueryClauseItem, QueryClauseItem, QueryResult};

/// Features that may be required by the Query Engine.
///
/// The query pipeline provides the language bindings a list of features that it can support, using these values.
/// The language binding can then forward that information to the gateway when generating a query plan, which allows the gateway to reject queries that the engine cannot support.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueryFeature {
    NoneQuery,
    Aggregate,
    CompositeAggregate,
    Distinct,
    GroupBy,
    MultipleAggregates,
    MultipleOrderBy,
    OffsetAndLimit,
    OrderBy,
    Top,
    NonStreamingOrderBy,
    HybridSearch,
    CountIf,
}

#[derive(Debug, Clone)]
pub struct Query {
    /// The text of the query.
    pub text: String,

    /// The parameters of the query, pre-encoded as a JSON object suitable to being the `parameters` field of a Cosmos query.
    pub encoded_parameters: Option<Box<serde_json::value::RawValue>>,
}

#[derive(Debug, Deserialize)]
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

/// Describes a request for additional data from the pipeline.
///
/// This value is returned when the pipeline needs more data to continue processing.
/// It contains the information necessary for the caller to make an HTTP request to the Cosmos APIs to fetch the next batch of data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "python_conversions", derive(pyo3::IntoPyObject))]
pub struct DataRequest {
    pub pkrange_id: Cow<'static, str>,
    pub continuation: Option<String>,
}

impl DataRequest {
    pub fn new(pkrange_id: impl Into<Cow<'static, str>>, continuation: Option<String>) -> Self {
        Self {
            pkrange_id: pkrange_id.into(),
            continuation,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "python_conversions", derive(pyo3::IntoPyObject))]
pub struct PipelineResponse<T> {
    /// The items returned by the pipeline.
    pub items: Vec<T>,

    /// Requests for additional data from the pipeline.
    ///
    /// If [`PipelineResponse::terminated`] is `true`, this will be empty and can be ignored.
    pub requests: Vec<DataRequest>,

    /// Indicates if the pipeline has terminated.
    ///
    /// If this is true, no further items will be produced, even if more data is provided.
    pub terminated: bool,
}

impl<T> PipelineResponse<T> {
    pub const TERMINATED: Self = Self {
        items: Vec::new(),
        requests: Vec::new(),
        terminated: true,
    };

    pub fn map_items<U, F>(self, f: F) -> PipelineResponse<U>
    where
        F: Fn(T) -> U,
    {
        PipelineResponse {
            items: self.items.into_iter().map(f).collect(),
            requests: self.requests,
            terminated: self.terminated,
        }
    }
}
