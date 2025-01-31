use std::collections::VecDeque;

use serde::Deserialize;

use crate::Result;

mod merge_strategy;
mod plan;
mod query_result;

use merge_strategy::MergeStrategy;

pub use plan::{QueryInfo, QueryPlan, QueryRange, SortOrder};
pub use query_result::{QueryClauseItem, QueryResult};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartitionKeyRange {
    id: String,
    min_inclusive: String,
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

/// Represents the current stage that a partition is in during the query.
enum PartitionStage {
    /// The partition is ready for the first data request. There should be no data in the queue yet.
    Initial,

    /// The partition has a pending continuation. When the current queue is exhausted, the continuation can be used to fetch more data.
    Continuing(String),

    /// The partition has been exhausted. When the current queue is exhausted, the partition is done.
    Done,
}

struct PartitionState {
    pkrange: PartitionKeyRange,
    queue: VecDeque<QueryResult>,
    stage: PartitionStage,
}

impl PartitionState {
    pub fn new(pkrange: PartitionKeyRange) -> Self {
        Self {
            pkrange,
            queue: VecDeque::new(),
            stage: PartitionStage::Initial,
        }
    }

    pub fn set_stage(&mut self, stage: PartitionStage) {
        self.stage = stage;
    }

    /// Returns a boolean indicating if the partition is exhausted (i.e. the queue is empty and the stage is `PartitionStage::Done`, so requesting more data will not produce any new data).
    pub fn exhausted(&self) -> bool {
        self.queue.is_empty() && matches!(self.stage, PartitionStage::Done)
    }

    pub fn enqueue(&mut self, item: QueryResult) {
        self.queue.push_back(item);
    }

    pub fn next_data_request(&self) -> Option<DataRequest> {
        todo!()
    }
}

pub struct QueryPipeline {
    partitions: Vec<PartitionState>,
    merge_strategy: MergeStrategy,
}

/// Describes a request for additional data from the pipeline.
///
/// This value is returned when the pipeline needs more data to continue processing.
/// It contains the information necessary for the caller to make an HTTP request to the Cosmos APIs to fetch the next batch of data.
pub struct DataRequest {}

/// The response from the query pipeline when requesting the next item.
pub enum PipelineResponse {
    /// The pipeline has insufficient data to complete this request.
    ///
    /// The receiver should issue all the HTTP requests described by the provided [`DataRequest`]s, provide the results to the pipeline, and then call [`QueryPipeline::next_batch`] again.
    MoreDataNeeded(Vec<DataRequest>),

    /// The pipeline has produced a batch of query results.
    ///
    /// The receiver should return these results to the user.
    Batch(Vec<QueryResult>),

    /// The pipeline has concluded processing and has no more results to produce.
    Done,
}

impl QueryPipeline {
    pub fn new(plan: QueryPlan, pkranges: impl Iterator<Item = PartitionKeyRange>) -> Self {
        let partitions = pkranges
            .map(|r| PartitionState {
                pkrange: r,
                queue: VecDeque::new(),
                stage: PartitionStage::Initial,
            })
            .collect();

        let merge_strategy = if plan.query_info.order_by.is_empty() {
            MergeStrategy::Unordered
        } else {
            MergeStrategy::Ordered(plan.query_info.order_by)
        };

        Self {
            partitions,
            merge_strategy,
        }
    }

    pub fn next_batch(&mut self) -> Result<PipelineResponse> {
        let mut batch = Vec::new();
        loop {
            let item = self.merge_strategy.next_item(&mut self.partitions)?;
            let Some(item) = item else {
                // We're done, return our current batch.
                break;
            };

            // TODO: "Pull" the item through the query pipeline, once we have one.

            batch.push(item);
        }

        if batch.is_empty() {
            let requests = self
                .partitions
                .iter()
                .filter_map(|p| p.next_data_request())
                .collect::<Vec<_>>();
            if requests.is_empty() {
                Ok(PipelineResponse::Done)
            } else {
                Ok(PipelineResponse::MoreDataNeeded(requests))
            }
        } else {
            Ok(PipelineResponse::Batch(batch))
        }
    }
}
