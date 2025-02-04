use crate::ErrorKind;

use super::{
    node::{LimitPipelineNode, OffsetPipelineNode, PipelineNode, PipelineResult, PipelineSlice},
    plan::DistinctType,
    producer::{ItemProducer, MergeStrategy},
    PartitionKeyRange, PipelineResponse, QueryFeature, QueryPlan, QueryResult,
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

pub struct QueryPipeline<T> {
    pipeline: Vec<Box<dyn PipelineNode<T>>>,
    producer: ItemProducer<T>,

    // Indicates if the pipeline has been terminated early.
    terminated: bool,
}

impl<T> QueryPipeline<T> {
    /// Creates a new query pipeline.
    ///
    /// # Parameters
    /// * `query` - The ORIGINAL query specified by the user. If the [`QueryPlan`] has a `rewritten_query`, the pipeline will handle rewriting it.
    /// * `plan` - The query plan that describes how to execute the query.
    /// * `pkranges` - An iterator that produces the [`PartitionKeyRange`]s that the query will be executed against.
    pub fn new(
        plan: QueryPlan,
        pkranges: impl IntoIterator<Item = PartitionKeyRange>,
    ) -> crate::Result<Self> {
        let merge_strategy = if plan.query_info.order_by.is_empty() {
            MergeStrategy::Unordered
        } else {
            if plan.query_info.has_non_streaming_order_by {
                return Err(ErrorKind::UnsupportedQueryPlan
                    .with_message("non-streaming ORDER BY queries are not supported"));
            }

            MergeStrategy::Ordered(plan.query_info.order_by)
        };

        let producer = ItemProducer::new(pkranges, merge_strategy);

        // We are building the pipeline outside-in.
        // That means the first node we push will be the first node executed.
        // This is relevant for nodes like OFFSET and LIMIT, which need to be ordered carefully.
        let mut pipeline: Vec<Box<dyn PipelineNode<T>>> = Vec::new();

        // We have to do limiting at right at the outside of the pipeline, so that OFFSET can skip items without affecting the LIMIT counter.
        if let Some(limit) = plan.query_info.limit {
            pipeline.push(Box::new(LimitPipelineNode::new(limit)));
        }

        if let Some(top) = plan.query_info.top {
            pipeline.push(Box::new(LimitPipelineNode::new(top)));
        }

        if let Some(offset) = plan.query_info.offset {
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

        Ok(Self {
            pipeline,
            producer,
            terminated: false,
        })
    }

    pub fn provide_data(
        &mut self,
        pkrange_id: &str,
        data: Vec<QueryResult<T>>,
        continuation: Option<String>,
    ) -> crate::Result<()> {
        self.producer.provide_data(pkrange_id, data, continuation)
    }

    /// Advances the pipeline to the next batch of results.
    ///
    /// This method will return a [`PipelineResponse`] that describes the next action to take.
    pub fn next_batch(&mut self) -> crate::Result<Option<PipelineResponse<T>>> {
        if self.terminated {
            return Ok(None);
        }

        let mut slice = PipelineSlice::new(&mut self.pipeline, &mut self.producer);

        let mut batch = Vec::new();
        loop {
            match slice.next_item()? {
                PipelineResult::Result(item) => batch.push(item.into_payload()),
                PipelineResult::EarlyTermination => {
                    self.terminated = true;

                    // We still need to emit any items in this batch.
                    break;
                }
                PipelineResult::NoResult => break,
            }
        }

        let requests = self.producer.data_requests();

        if batch.is_empty() && requests.is_empty() {
            // We're done!
            Ok(None)
        } else {
            Ok(Some(PipelineResponse { batch, requests }))
        }
    }
}

// The tests for the pipeline are found in integration tests (in the `tests`) directory, since we want to test an end-to-end experience that matches what the user will see.
// Individual components of the pipeline are tested in the other modules.
