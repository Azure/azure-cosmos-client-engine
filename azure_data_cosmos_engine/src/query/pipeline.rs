// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use std::ffi::CStr;

use crate::{
    query::{node::AggregatePipelineNode, query_result::QueryResultShape},
    ErrorKind,
};

use super::{
    node::{LimitPipelineNode, OffsetPipelineNode, PipelineNode, PipelineSlice},
    plan::{DistinctType, QueryRange},
    producer::ItemProducer,
    PartitionKeyRange, PipelineResponse, QueryFeature, QueryPlan, QueryResult,
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

supported_features!(
    OffsetAndLimit,
    OrderBy,
    MultipleOrderBy,
    Top,
    NonStreamingOrderBy,
    Aggregate
);

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
pub struct QueryPipeline {
    query: String,
    pipeline: Vec<Box<dyn PipelineNode>>,
    producer: ItemProducer,
    result_shape: QueryResultShape,

    // Indicates if the pipeline has been terminated early.
    terminated: bool,
}

impl QueryPipeline {
    /// Creates a new query pipeline.
    ///
    /// # Parameters
    /// * `query` - The ORIGINAL query specified by the user. If the [`QueryPlan`] has a `rewritten_query`, the pipeline will handle rewriting it.
    /// * `plan` - The query plan that describes how to execute the query.
    /// * `pkranges` - An iterator that produces the [`PartitionKeyRange`]s that the query will be executed against.
    #[tracing::instrument(level = "debug", skip_all, err)]
    pub fn new(
        query: &str,
        plan: QueryPlan,
        pkranges: impl IntoIterator<Item = PartitionKeyRange>,
    ) -> crate::Result<Self> {
        let mut pkranges: Vec<PartitionKeyRange> = pkranges.into_iter().collect();
        get_overlapping_pk_ranges(&mut pkranges, &plan.query_ranges);

        let mut result_shape = QueryResultShape::RawPayload;

        tracing::trace!(?query, ?plan, "creating query pipeline");

        // We don't support non-value aggregates, so make sure the query doesn't have any.
        if !plan.query_info.aggregates.is_empty() && !plan.query_info.has_select_value {
            return Err(ErrorKind::UnsupportedQueryPlan
                .with_message("non-value aggregates are not supported"));
        }

        let producer = if plan.query_info.order_by.is_empty() {
            tracing::debug!("using unordered pipeline");
            ItemProducer::unordered(pkranges)
        } else {
            result_shape = QueryResultShape::OrderBy;
            if plan.query_info.has_non_streaming_order_by {
                tracing::debug!(?plan.query_info.order_by, "using non-streaming ORDER BY pipeline");
                ItemProducer::non_streaming(pkranges, plan.query_info.order_by)
            } else {
                // We can stream results, there's no vector or full-text search in the query.
                tracing::debug!(?plan.query_info.order_by, "using streaming ORDER BY pipeline");
                ItemProducer::streaming(pkranges, plan.query_info.order_by)
            }
        };

        // We are building the pipeline outside-in.
        // That means the first node we push will be the first node executed.
        // This is relevant for nodes like OFFSET and LIMIT, which need to be ordered carefully.
        let mut pipeline: Vec<Box<dyn PipelineNode>> = Vec::new();

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

        if !plan.query_info.aggregates.is_empty() {
            if result_shape != QueryResultShape::RawPayload {
                return Err(ErrorKind::UnsupportedQueryPlan
                    .with_message("cannot mix aggregates with ORDER BY"));
            }
            result_shape = QueryResultShape::ValueAggregate;
            pipeline.push(Box::new(AggregatePipelineNode::from_names(
                plan.query_info.aggregates.clone(),
            )?));
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
            result_shape,
            pipeline,
            producer,
            terminated: false,
        })
    }

    /// Retrieves the shape of the results produced by this pipeline.
    /// The shape determines how the pipeline deserializes results from single-partition queries.
    pub fn result_shape(&self) -> &QueryResultShape {
        &self.result_shape
    }

    /// Retrieves the, possibly rewritten, query that this pipeline is executing.
    ///
    /// The pipeline has both the original query, AND the query plan that may have rewritten it.
    /// So, no matter whether or not the query was rewritten, this query will be accurate.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Indicates if the pipeline has been completed.
    pub fn complete(&self) -> bool {
        self.terminated
    }

    /// Provides more data for the specified partition key range.
    #[tracing::instrument(level = "debug", skip_all, err, fields(pkrange_id = pkrange_id, data_len = data.len(), continuation = continuation.as_deref()))]
    pub fn provide_data(
        &mut self,
        pkrange_id: &str,
        data: Vec<QueryResult>,
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
    #[tracing::instrument(level = "debug", skip(self), err)]
    pub fn run(&mut self) -> crate::Result<PipelineResponse> {
        if self.terminated {
            return Ok(PipelineResponse::TERMINATED);
        }

        let mut slice = PipelineSlice::new(&mut self.pipeline, &mut self.producer);

        let mut items = Vec::new();
        while !self.terminated {
            let result = slice.run()?;

            // Termination MUST come from the pipeline, to ensure aggregates (which can only be emitted after all data is processed) work correctly.
            if result.terminated {
                tracing::trace!("pipeline node terminated the pipeline");
                self.terminated = true;
            }

            if let Some(item) = result.value {
                // TODO: Handle scenarios where there is no payload (aggregates)
                items.push(item.payload.unwrap());
            } else {
                // The pipeline has finished for now, but we're not terminated yet.
                break;
            }
        }

        let requests = self.producer.data_requests();

        Ok(PipelineResponse {
            items,
            requests,
            terminated: self.terminated,
        })
    }
}

/// Rewrites the incoming query by replacing tokens within it.
fn format_query(original: &str) -> String {
    original.replace("{documentdb-formattableorderbyquery-filter}", "true")
}

/// Filters the partition key ranges to include only those that overlap with the query ranges.
/// If no query ranges are provided, all partition key ranges are retained.
fn get_overlapping_pk_ranges(pkranges: &mut Vec<PartitionKeyRange>, query_ranges: &[QueryRange]) {
    if query_ranges.is_empty() {
        return;
    }

    debug_assert!(
        pkranges.is_sorted_by_key(|pkrange| pkrange.min_inclusive.clone()),
        "partition key ranges must be sorted by minInclusive"
    );

    pkranges.retain(|pkrange| {
        query_ranges.iter().any(|query_range| {
            ranges_overlap(
                &pkrange.min_inclusive,
                &pkrange.max_exclusive,
                true,  // PK ranges are always min-inclusive
                false, // PK ranges are always max-exclusive
                &query_range.min,
                &query_range.max,
                query_range.is_min_inclusive,
                query_range.is_max_inclusive,
            )
        })
    });
}

/// Determines if two ranges overlap.
fn ranges_overlap(
    range1_min: &str,
    range1_max: &str,
    range1_min_inclusive: bool,
    range1_max_inclusive: bool,
    range2_min: &str,
    range2_max: &str,
    range2_min_inclusive: bool,
    range2_max_inclusive: bool,
) -> bool {
    // Check if ranges don't overlap (easier to reason about)
    let no_overlap = if range1_max < range2_min {
        true
    } else if range1_max == range2_min {
        !(range1_max_inclusive && range2_min_inclusive)
    } else if range2_max < range1_min {
        true
    } else if range2_max == range1_min {
        !(range2_max_inclusive && range1_min_inclusive)
    } else {
        false
    };

    !no_overlap
}

// The tests for the pipeline are found in integration tests (in the `tests`) directory, since we want to test an end-to-end experience that matches what the user will see.
// Individual components of the pipeline are tested in the other modules.
