use std::{
    borrow::Cow,
    collections::{BTreeMap, VecDeque},
};

use serde::Deserialize;

mod merge_strategy;
mod plan;
mod query_result;

use merge_strategy::MergeStrategy;

pub use plan::{QueryInfo, QueryPlan, QueryRange, SortOrder};
pub use query_result::{QueryClauseItem, QueryResult};

use crate::ErrorKind;

#[derive(Debug, Clone)]
pub struct Query {
    /// The text of the query.
    pub text: String,

    /// The parameters of the query, pre-encoded as a JSON object suitable to being the `parameters` field of a Cosmos query.
    pub encoded_parameters: Option<Box<serde_json::value::RawValue>>,
}

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

    /// Returns a boolean indicating if the partition is exhausted (i.e. the queue is empty and the stage is `PartitionStage::Done`, so requesting more data will not produce any new data).
    pub fn exhausted(&self) -> bool {
        self.queue.is_empty() && matches!(self.stage, PartitionStage::Done)
    }

    pub fn enqueue(&mut self, item: QueryResult) {
        self.queue.push_back(item);
    }

    pub fn extend(
        &mut self,
        item: impl IntoIterator<Item = QueryResult>,
        continuation: Option<String>,
    ) {
        self.queue.extend(item);
        self.stage = continuation.map_or_else(
            || PartitionStage::Done,
            |token| PartitionStage::Continuing(token),
        );
    }

    pub fn next_data_request(&self) -> Option<DataRequest> {
        // If the queue is not empty, we don't need to request more data.
        if !self.queue.is_empty() {
            return None;
        }

        match &self.stage {
            PartitionStage::Initial => Some(DataRequest {
                pkrange_id: self.pkrange.id.clone().into(),
                continuation: None,
            }),
            PartitionStage::Continuing(token) => Some(DataRequest {
                pkrange_id: self.pkrange.id.clone().into(),
                continuation: Some(token.clone()),
            }),
            PartitionStage::Done => None,
        }
    }

    pub fn has_started(&self) -> bool {
        !matches!(self.stage, PartitionStage::Initial)
    }
}

/// Describes a request for additional data from the pipeline.
///
/// This value is returned when the pipeline needs more data to continue processing.
/// It contains the information necessary for the caller to make an HTTP request to the Cosmos APIs to fetch the next batch of data.
#[derive(Debug, PartialEq, Eq)]
pub struct DataRequest {
    pkrange_id: Cow<'static, str>,
    continuation: Option<String>,
}

impl DataRequest {
    pub fn new(pkrange_id: impl Into<Cow<'static, str>>, continuation: Option<String>) -> Self {
        Self {
            pkrange_id: pkrange_id.into(),
            continuation,
        }
    }
}

/// The response from the query pipeline when requesting the next item.
#[derive(Debug)]
pub enum PipelineResponse {
    // We could probably collapse these a bit, since often more data will be needed after a batch is provided.
    // But for now, we're keeping them separate to keep things clear and simple. Optimization can come later and be done without impacting language SDKs.
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

pub struct QueryPipeline {
    query: Query,
    partitions: Vec<PartitionState>,
    merge_strategy: MergeStrategy,
}

impl QueryPipeline {
    /// Creates a new query pipeline.
    ///
    /// # Parameters
    /// * `query` - The ORIGINAL query specified by the user. If the [`QueryPlan`] has a `rewritten_query`, the pipeline will handle rewriting it.
    /// * `plan` - The query plan that describes how to execute the query.
    /// * `pkranges` - An iterator that produces the [`PartitionKeyRange`]s that the query will be executed against.
    pub fn new(
        query: Query,
        plan: QueryPlan,
        pkranges: impl IntoIterator<Item = PartitionKeyRange>,
    ) -> Self {
        let partitions = pkranges
            .into_iter()
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
            query,
            partitions,
            merge_strategy,
        }
    }

    pub fn provide_data(
        &mut self,
        pkrange_id: &str,
        data: Vec<QueryResult>,
        continuation: Option<String>,
    ) -> crate::Result<()> {
        // We currently store partitions as a Vec, so we need to search to find the partition to update.
        // The number of PK ranges is expected to be "small" (certainly compared to the item count), so this is not a performance concern right now.
        // If it becomes a concern, we can use a BTreeMap keyed by PK range ID (this has some, solvable, implications to the merge strategy, which is why we haven't done it yet).
        let partition = self
            .partitions
            .iter_mut()
            .find(|p| p.pkrange.id == pkrange_id)
            .ok_or_else(|| {
                ErrorKind::UnknownPartitionKeyRange
                    .with_message(format!("unknown partition key range ID: {}", pkrange_id))
            })?;

        partition.extend(data, continuation);

        Ok(())
    }

    /// Advances the pipeline to the next batch of results.
    ///
    /// This method will return a [`PipelineResponse`] that describes the next action to take.
    pub fn next_batch(&mut self) -> crate::Result<PipelineResponse> {
        // TODO: Run each item through a pipeline
        let item_iter = self.merge_strategy.item_iter(&mut self.partitions);
        let batch = item_iter.collect::<crate::Result<Vec<_>>>()?;

        if batch.is_empty() {
            // If there are no items in the batch, we need to request more data.
            let requests = self
                .partitions
                .iter()
                .filter_map(|p| p.next_data_request())
                .collect::<Vec<_>>();

            // If there were no outstanding requests, then we are done.
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

#[cfg(test)]
mod test {
    use serde::Serialize;

    use super::*;

    /// Our version of the nightly-only [`assert_matches`](std::assert_matches) macro.
    ///
    /// Asserts that the provided value matches the pattern, and runs the provided expression if it does, returning that result.
    macro_rules! assert_match {
        ($e: expr, $p: pat) => {
            match $e {
                $p => (),
                _ => panic!("Expected {}, got {:?}", stringify!($p), $e),
            }
        };
        ($e: expr, $p: pat => $ret: expr) => {
            match $e {
                $p => $ret,
                _ => panic!("Expected {}, got {:?}", stringify!($p), $e),
            }
        };
    }

    #[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
    pub struct Item {
        id: String,
        pk: String,
        title: String,
    }

    impl Item {
        pub fn new(id: impl Into<String>, pk: impl Into<String>, title: impl Into<String>) -> Self {
            Self {
                id: id.into(),
                pk: pk.into(),
                title: title.into(),
            }
        }
    }

    fn create_item(
        partition_id: &str,
        id: String,
        order_by_items: Vec<serde_json::Value>,
    ) -> QueryResult {
        let item = Item::new(
            id.clone(),
            partition_id.to_string(),
            format!("{} / {}", partition_id, id),
        );
        let s = serde_json::to_string(&item).unwrap();
        let raw = serde_json::value::RawValue::from_string(s).unwrap();
        let order_by_items = order_by_items
            .into_iter()
            .map(|value| serde_json::from_value(value).unwrap())
            .collect();
        QueryResult::new(vec![], order_by_items, raw)
    }

    fn create_items(partition_id: &str, start_id: usize, count: usize) -> Vec<QueryResult> {
        (0..count)
            .map(|i| {
                let id = format!("item{}", start_id + i);
                create_item(partition_id, id, Vec::new())
            })
            .collect()
    }

    fn create_pipeline() -> QueryPipeline {
        let plan = QueryPlan {
            partitioned_query_execution_info_version: 1,
            query_info: QueryInfo {
                distinct_type: "None".into(),
                order_by: vec![],
                order_by_expressions: vec![],
                rewritten_query: "".into(),
            },
            query_ranges: vec![],
        };
        let query = Query {
            text: "SELECT * FROM c".into(),
            encoded_parameters: None,
        };
        QueryPipeline::new(
            query,
            plan,
            [
                PartitionKeyRange::new("partition0", "00", "99"),
                PartitionKeyRange::new("partition1", "99", "FF"),
            ],
        )
    }

    fn drain_pipeline(pipeline: &mut QueryPipeline) -> crate::Result<Vec<Item>> {
        let batch = assert_match!(pipeline.next_batch()?, PipelineResponse::Batch(e) => e);
        batch
            .iter()
            .map(|r| r.payload_into())
            .collect::<Result<Vec<Item>, _>>()
    }

    // We're making a tactical choice here not to test _every_ possible permutation of the pipeline here.
    // The Merge Strategy is well tested, and each node type will be tested in isolation.
    // Here, we're largely testing the batching, orchestration (running items through the pipeline), state management, and pipeline construction logic.

    #[test]
    pub fn next_batch_no_nodes() -> Result<(), Box<dyn std::error::Error>> {
        let mut pipeline = create_pipeline();

        // First call should ask for more data from all partitions
        let requests =
            assert_match!(pipeline.next_batch()?, PipelineResponse::MoreDataNeeded(e) => e);
        assert_eq!(
            vec![
                DataRequest::new("partition0", None),
                DataRequest::new("partition1", None),
            ],
            requests
        );

        // Now insert some data, and a continuation token for each partition.
        pipeline.provide_data(
            "partition0",
            create_items("partition0", 0, 3),
            Some("p1c0".into()),
        )?;
        pipeline.provide_data(
            "partition1",
            create_items("partition1", 0, 3),
            Some("p1c0".into()),
        )?;

        // We should get a single batch with only the items in partition 0, but none from partition 1 (because there may be more data in partition 0)
        let batch = drain_pipeline(&mut pipeline)?;
        assert_eq!(
            vec![
                Item::new("item0", "partition0", "partition0 / item0"),
                Item::new("item1", "partition0", "partition0 / item1"),
                Item::new("item2", "partition0", "partition0 / item2"),
            ],
            batch
        );

        // We can now add some more data for partition 0, but mark it done.
        pipeline.provide_data("partition0", create_items("partition0", 3, 3), None)?;

        // Now we should get all the remaining data from partition 0, and the items from partition 1.
        let batch = drain_pipeline(&mut pipeline)?;
        assert_eq!(
            vec![
                Item::new("item3", "partition0", "partition0 / item3"),
                Item::new("item4", "partition0", "partition0 / item4"),
                Item::new("item5", "partition0", "partition0 / item5"),
                Item::new("item0", "partition1", "partition1 / item0"),
                Item::new("item1", "partition1", "partition1 / item1"),
                Item::new("item2", "partition1", "partition1 / item2"),
            ],
            batch
        );

        // Completing partition 1 should complete the entire pipeline.
        pipeline.provide_data("partition1", vec![], None)?;
        assert_match!(pipeline.next_batch()?, PipelineResponse::Done);

        Ok(())
    }
}
