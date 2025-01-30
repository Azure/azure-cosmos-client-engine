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

struct PartitionState {
    pkrange: PartitionKeyRange,
    queue: VecDeque<QueryResult>,
}

pub struct QueryPipeline {
    partitions: Vec<PartitionState>,
    merge_strategy: MergeStrategy,
}

impl QueryPipeline {
    pub fn new(plan: QueryPlan, pkranges: impl Iterator<Item = PartitionKeyRange>) -> Self {
        let partitions = pkranges
            .map(|r| PartitionState {
                pkrange: r,
                queue: VecDeque::new(),
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

    pub fn next_item(&mut self) -> Result<Option<QueryResult>> {
        let next_partition = self.merge_strategy.next_partition(&mut self.partitions)?;
        let next_item = next_partition.and_then(|p| p.queue.pop_front());
        Ok(next_item)
    }
}
