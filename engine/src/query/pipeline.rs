use std::fmt::Debug;

use serde::{de::DeserializeOwned, Deserialize};

use crate::ErrorKind;

use super::{
    node::{
        LimitPipelineNode, OffsetPipelineNode, PipelineNode, PipelineNodeResult, PipelineSlice,
    },
    plan::DistinctType,
    producer::{ItemProducer, MergeStrategy},
    PartitionKeyRange, PipelineResponse, QueryClauseItem, QueryFeature, QueryPlan, QueryResult,
};

macro_rules! supported_features {
    ($($feature:ident),*) => {
        pub const SUPPORTED_FEATURES: &'static [QueryFeature] = &[$(QueryFeature::$feature),*];
        pub const SUPPORTED_FEATURES_STRING: &'static str = concat!($(
            stringify!($feature), ","
        ),*);
    };
}

supported_features!(OffsetAndLimit, OrderBy, MultipleOrderBy, Top);

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
            rewrite_query(&plan.query_info.rewritten_query)
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
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Provides more data for the specified partition key range.
    #[tracing::instrument(level = "debug", skip(self), fields(pkrange_id = pkrange_id))]
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
    #[tracing::instrument(level = "debug", skip(self))]
    pub fn next_batch(&mut self) -> crate::Result<Option<PipelineResponse<T>>> {
        if self.terminated {
            return Ok(None);
        }

        let mut slice = PipelineSlice::new(&mut self.pipeline, &mut self.producer);

        let mut items = Vec::new();
        loop {
            match slice.next_item()? {
                PipelineNodeResult::Result(item) => items.push(item.into_payload()),
                PipelineNodeResult::EarlyTermination => {
                    self.terminated = true;

                    // We still need to emit any items in this batch.
                    break;
                }
                PipelineNodeResult::NoResult => break,
            }
        }

        let requests = self.producer.data_requests();

        if items.is_empty() && requests.is_empty() {
            // We're done!
            Ok(None)
        } else {
            Ok(Some(PipelineResponse { items, requests }))
        }
    }

    pub(crate) fn results_are_bare_payloads(&self) -> bool {
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

fn rewrite_query(original: &str) -> String {
    let rewritten = original.replace("{documentdb-formattableorderbyquery-filter}", "true");
    tracing::debug!(
        ?original,
        ?rewritten,
        "rewrote query, per gateway query plan"
    );
    rewritten
}

// The tests for the pipeline are found in integration tests (in the `tests`) directory, since we want to test an end-to-end experience that matches what the user will see.
// Individual components of the pipeline are tested in the other modules.
