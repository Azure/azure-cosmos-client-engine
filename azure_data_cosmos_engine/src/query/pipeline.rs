use std::{ffi::CStr, fmt::Debug};

use serde::{de::DeserializeOwned, Deserialize};

use crate::ErrorKind;

use super::{
    node::{LimitPipelineNode, OffsetPipelineNode, PipelineNode, PipelineSlice},
    plan::DistinctType,
    producer::{ItemProducer, MergeStrategy},
    PartitionKeyRange, PipelineResponse, QueryClauseItem, QueryFeature, QueryPlan, QueryResult,
};

/// Holds a list of [`QueryFeature`]s and a string representation suitable for being passed to the gateway when requesting a query plan.
pub struct SupportedFeatures {
    #[allow(dead_code)]
    supported_features: &'static [QueryFeature],
    supported_features_cstr: &'static CStr,
}

impl SupportedFeatures {
    /// Gets a slice of [`QueryFeature`] values representing the features supported by this engine.
    pub const fn as_slice(&self) -> &'static [QueryFeature] {
        self.supported_features
    }

    /// Gets a Rust string representing the supported features, suitable for being passed to the gateway when requesting a query plan.
    pub const fn as_str(&self) -> &'static str {
        match self.supported_features_cstr.to_str() {
            Ok(s) => s,
            Err(_) => panic!("supported_features_cstr is not valid UTF-8"),
        }
    }

    /// Gets a C string representing the supported features, suitable for being passed to the gateway when requesting a query plan.
    pub const fn as_cstr(&self) -> &'static CStr {
        self.supported_features_cstr
    }
}

macro_rules! supported_features {
    ($($feature:ident),*) => {
        #[doc = "A [`SupportedFeatures`](SupportedFeatures) describing the features supported by this query engine."]
        pub const SUPPORTED_FEATURES: SupportedFeatures = SupportedFeatures {
            supported_features: &[$(QueryFeature::$feature),*],
            supported_features_cstr: make_cstr!(concat!($(
                stringify!($feature), ","
            ),*)),
        };
    };
}

supported_features!(OffsetAndLimit, OrderBy, MultipleOrderBy, Top);

/// Represents a query pipeline capable of accepting single-partition results for a query and returning a cross-partition stream of results.
///
/// ## Overview
///
/// The [`QueryPipeline`] is the core of the Cosmos Client Engine's query engine.
/// To perform a cross-partition query, a client has to perform separate queries against each individual partition, then aggregate the results.
/// This aggregation process is non-trivial, it requires processing the incoming data and handling any `ORDER BY`, `GROUP BY`, etc. clauses to ensure accurate results.
///
/// For example, consider the query `SELECT * FROM c ORDER BY c.foo`, where `foo` is not the partition key.
/// To execute this query correctly, a client must:
///
/// 1. Parse the query into a query plan, to identify that it contains an `ORDER BY` operation and what property is being ordered.
/// 2. Fetch the list of Partition Key Ranges (PK Ranges) for the container.
/// 3. Execute the query separately against each PK Range, retrieving a set of _single-partition_ results that are each correctly ordered.
/// 4. Merge the separate single-partition result streams into a single stream, respecting the ordering as you go.
///
/// The first stage, parsing the query into a query plan, can be performed using the Gateway REST API.
/// Issuing a query request with the `x-ms-cosmos-is-query-plan-request` header set to `true` will cause the Gateway to return a query plan in JSON form.
/// The [`QueryPlan`] type can be deserialized from this type.
///
/// The second stage, fetching PK Ranges, can be performed using a call to the `/dbs/{dbname}/colls/{containername}/pkranges` REST API.
/// The [`PartitionKeyRange`] type can be deserialized from each returned PK Range.
///
/// The third stage can be performed by the per-language client, by executing the query using the Gateway REST API and specifying the `x-ms-documentdb-partitionkeyrangeid` header.
/// The response to this request will be the single-partition results for the query.
///
/// The fourth stage is what the [`QueryPipeline`] handles.
/// The pipeline accepts the query plan and partition key ranges as input.
/// This allows the pipeline to set up the state for tracking results from each individual partitions.
///
/// From there, the pipeline operates in "turns".
/// The language binding executes a turn by calling [`QueryPipeline::run`], which returns a [`PipelineResponse`] describing how to proceed.
/// See the documentation for [`QueryPipeline::run`] for more information on turns.
///
/// ## Query Rewriting
///
/// While the language binding has the original query provided by the user, the Gateway may rewrite it while generating a query plan.
/// The [`QueryInfo::rewritten_query`](crate::query::QueryInfo::rewritten_query) value, included in the query plan returned by the Gateway, includes that rewritten query.
/// Since most consumers of the pipeline don't actually parse the the query plan (instead, they pass the plan in as a string), the
/// pipeline exposes the rewritten query through the [`QueryPipeline::query()`] method.
/// If the query was *not* rewritten by the gateway, this method returns the unrewritten query,
/// so language bindings should *always* use this query when making the signal-partition queries.
pub struct QueryPipeline<T: Debug, I: QueryClauseItem> {
    query: String,
    pipeline: Vec<Box<dyn PipelineNode<T, I>>>,
    producer: ItemProducer<T, I>,
    results_are_bare_payloads: bool,

    // Indicates if the pipeline has been terminated early.
    terminated: bool,
}

impl<T: Debug, I: QueryClauseItem> QueryPipeline<T, I> {
    /// Creates a new query pipeline.
    ///
    /// # Parameters
    /// * `query` - The ORIGINAL query specified by the user. If the [`QueryPlan`] has a `rewritten_query`, the pipeline will handle rewriting it.
    /// * `plan` - The query plan that describes how to execute the query.
    /// * `pkranges` - An iterator that produces the [`PartitionKeyRange`]s that the query will be executed against.
    pub fn new(
        query: &str,
        plan: QueryPlan,
        pkranges: impl IntoIterator<Item = PartitionKeyRange>,
    ) -> crate::Result<Self> {
        let mut results_are_bare_payloads = true;

        let merge_strategy = if plan.query_info.order_by.is_empty() {
            tracing::debug!("using unordered merge strategy");
            MergeStrategy::Unordered
        } else {
            if plan.query_info.has_non_streaming_order_by {
                return Err(ErrorKind::UnsupportedQueryPlan
                    .with_message("non-streaming ORDER BY queries are not supported"));
            }

            tracing::debug!(?plan.query_info.order_by, "using ORDER BY merge strategy");
            results_are_bare_payloads = false;
            MergeStrategy::Ordered(plan.query_info.order_by)
        };

        let producer = ItemProducer::new(pkranges, merge_strategy);

        // We are building the pipeline outside-in.
        // That means the first node we push will be the first node executed.
        // This is relevant for nodes like OFFSET and LIMIT, which need to be ordered carefully.
        let mut pipeline: Vec<Box<dyn PipelineNode<T, I>>> = Vec::new();

        // We have to do limiting at right at the outside of the pipeline, so that OFFSET can skip items without affecting the LIMIT counter.
        if let Some(limit) = plan.query_info.limit {
            tracing::debug!(limit, "adding LIMIT node to pipeline");
            pipeline.push(Box::new(LimitPipelineNode::new(limit)));
        }

        if let Some(top) = plan.query_info.top {
            tracing::debug!(top, "adding TOP node to pipeline");
            pipeline.push(Box::new(LimitPipelineNode::new(top)));
        }

        if let Some(offset) = plan.query_info.offset {
            tracing::debug!(offset, "adding OFFSET node to pipeline");
            pipeline.push(Box::new(OffsetPipelineNode::new(offset)));
        }

        if plan.query_info.has_select_value {
            return Err(ErrorKind::UnsupportedQueryPlan
                .with_message("SELECT VALUE queries are not supported"));
        }

        if !plan.query_info.aggregates.is_empty() {
            return Err(
                ErrorKind::UnsupportedQueryPlan.with_message("aggregates are not supported")
            );
        }
        if !plan.query_info.group_by_expressions.is_empty()
            || !plan.query_info.group_by_alias_to_aggregate_type.is_empty()
            || !plan.query_info.group_by_aliases.is_empty()
        {
            return Err(
                ErrorKind::UnsupportedQueryPlan.with_message("GROUP BY queries are not supported")
            );
        }

        if plan.query_info.distinct_type != DistinctType::None {
            return Err(
                ErrorKind::UnsupportedQueryPlan.with_message("DISTINCT queries are not supported")
            );
        }

        let query = if plan.query_info.rewritten_query.is_empty() {
            query.to_string()
        } else {
            let rewritten = format_query(&plan.query_info.rewritten_query);
            tracing::debug!(
                original = ?query,
                ?rewritten,
                "rewrote query, per gateway query plan"
            );
            rewritten
        };

        Ok(Self {
            query,
            results_are_bare_payloads,
            pipeline,
            producer,
            terminated: false,
        })
    }

    /// Retrieves the, possibly rewritten, query that this pipeline is executing.
    ///
    /// The pipeline has both the original query, AND the query plan that may have rewritten it.
    /// So, no matter whether or not the query was rewritten, this query will be accurate.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Provides more data for the specified partition key range.
    #[tracing::instrument(level = "debug", skip_all, fields(pkrange_id = pkrange_id, data_len = data.len(), continuation = continuation.as_deref()))]
    pub fn provide_data(
        &mut self,
        pkrange_id: &str,
        data: Vec<QueryResult<T, I>>,
        continuation: Option<String>,
    ) -> crate::Result<()> {
        self.producer.provide_data(pkrange_id, data, continuation)
    }

    /// Advances the pipeline to the next batch of results.
    ///
    /// This method will return a [`PipelineResponse`] that describes the next action to take.
    ///
    /// 1. A list of results yielded by that turn, if any.
    /// 2. A list of requests for additional data from certain partitions, if any.
    ///
    /// The results provided represent the next set of results to be returned to the user.
    /// The language binding can return these to the user immediately.
    ///
    /// The requests provided describe any additional single-partition queries that must be completed in order to get more data.
    /// The language binding MUST perform ALL the provided requests before the pipeline will be able to yield additional results.
    /// The language binding MAY execute additional turns without having satisfied all the requests, and the pipeline will continue
    /// to return any requests that still need to be made.
    ///
    /// If the pipeline returns no items and no requests, then the query has completed and there are no further results to return.
    #[tracing::instrument(level = "debug", skip(self))]
    pub fn run(&mut self) -> crate::Result<PipelineResponse<T>> {
        if self.terminated {
            return Ok(PipelineResponse::TERMINATED);
        }

        let mut slice = PipelineSlice::new(&mut self.pipeline, &mut self.producer);

        let mut items = Vec::new();
        while !self.terminated {
            let result = slice.run()?;
            if result.terminated {
                self.terminated = true;
            }

            if let Some(item) = result.value {
                items.push(item.into_payload());
            } else {
                // The pipeline has finished for now, but we're not terminated yet.
                break;
            }
        }

        let requests = self.producer.data_requests();

        // Once there are no more requests, there's no more data to be provided.
        if requests.is_empty() {
            self.terminated = true;
        }

        Ok(PipelineResponse {
            items,
            requests,
            terminated: self.terminated,
        })
    }

    /// Returns a boolean indicating if the results of the single-partition queries are expected to be "bare" payloads.
    ///
    /// This is a somewhat esoteric API used only in a few language bindings.
    /// Let's start by addressing what a "bare" payload means.
    ///
    /// Consider a query like: `SELECT * FROM c`.
    /// This query does not need to be rewritten by the gateway,
    /// so the results of each single-partition query will be exactly what the user expects.
    /// This is a "bare" payload.
    /// The values returned by the query are exactly the payload the user expects with no wrapping.
    ///
    /// However, consider the query `SELECT * FROM c ORDER BY c.foo`.
    /// The gateway rewrites this into a single-partition query like this: `SELECT * as payload, '[{"item": c.foo}]' as orderByItems FROM c ORDER BY c.foo`.
    /// It does this so that the pipeline can ALWAYS extract the values to be ordered by reading `orderByItems` from the results without having to traverse the object to find the order by expressions.
    /// This is a "wrapped" payload, where the actual value the user wants returned is "wrapped" by an object that contains other metadata for the query pipeline.
    ///
    /// The [`QueryResult`] type can be deserialized from BOTH "bare" and "wrapped" payloads.
    /// Deserializing from a bare payload is done through [`QueryResult::from_payload`], but
    /// deserializing from a wrapped payload is done through [`QueryResult::deserialize`].
    /// So, this method identifies which deserialization method is necessary to get a [`QueryResult`]
    pub fn results_are_bare_payloads(&self) -> bool {
        self.results_are_bare_payloads
    }
}

impl<T: Debug + DeserializeOwned, I: QueryClauseItem + DeserializeOwned + Default>
    QueryPipeline<T, I>
{
    /// Deserializes the payload of a query result, according to the expectations of the query plan.
    ///
    /// The query plan can affect the format of the returned data, so this method will deserialize the payload accordingly.
    pub fn deserialize_payload(&self, input: &str) -> crate::Result<Vec<QueryResult<T, I>>> {
        #[derive(Deserialize)]
        struct DocumentResult<T> {
            #[serde(rename = "Documents")]
            documents: Vec<T>,
        }

        if self.results_are_bare_payloads {
            let results = serde_json::from_str::<DocumentResult<T>>(input)
                .map_err(|e| ErrorKind::InvalidGatewayResponse.with_source(e))?;
            Ok(results
                .documents
                .into_iter()
                .map(|doc| QueryResult::from_payload(doc))
                .collect())
        } else {
            let results = serde_json::from_str::<DocumentResult<_>>(input)
                .map_err(|e| ErrorKind::InvalidGatewayResponse.with_source(e))?;
            Ok(results.documents)
        }
    }
}

/// Rewrites the incoming query by replacing tokens within it.
fn format_query(original: &str) -> String {
    let rewritten = original.replace("{documentdb-formattableorderbyquery-filter}", "true");
    rewritten
}

// The tests for the pipeline are found in integration tests (in the `tests`) directory, since we want to test an end-to-end experience that matches what the user will see.
// Individual components of the pipeline are tested in the other modules.
