use serde::Deserialize;

mod plan;

pub use plan::{QueryInfo, QueryPlan, QueryRange, SortOrder};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartitionKeyRange {
    id: String,
    min_inclusive: String,
    max_exclusive: String,
}

struct PartitionState {
    pkrange: PartitionKeyRange,
}

pub struct QueryPipeline {
    partitions: Vec<PartitionState>,
}

impl QueryPipeline {
    pub fn new(plan: QueryPlan, pkranges: impl Iterator<Item = PartitionKeyRange>) -> Self {
        let partitions = pkranges.map(|r| PartitionState { pkrange: r }).collect();

        // Build the pipeline from the query plan

        Self { partitions }
    }
}

impl Drop for QueryPipeline {
    fn drop(&mut self) {
        // Since memory management can be a concern, we report when the QueryPipeline is dropped to the tracing subscriber, if there is one.
        tracing::debug!("QueryPipeline dropped");
    }
}
