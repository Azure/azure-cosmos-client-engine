use std::borrow::Cow;

use node::{LimitPipelineNode, PipelineNode, PipelineResult, PipelineSlice};
use serde::Deserialize;

pub mod node;
mod plan;
mod producer;
mod query_result;

use producer::{ItemProducer, MergeStrategy};

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

/// Describes a request for additional data from the pipeline.
///
/// This value is returned when the pipeline needs more data to continue processing.
/// It contains the information necessary for the caller to make an HTTP request to the Cosmos APIs to fetch the next batch of data.
#[derive(Debug, PartialEq, Eq)]
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

/// The response from the query pipeline when requesting the next item.
#[derive(Debug)]
pub enum PipelineResponse<T> {
    // We could probably collapse these a bit, since often more data will be needed after a batch is provided.
    // But for now, we're keeping them separate to keep things clear and simple. Optimization can come later and be done without impacting language SDKs.
    /// The pipeline has insufficient data to complete this request.
    ///
    /// The receiver should issue all the HTTP requests described by the provided [`DataRequest`]s, provide the results to the pipeline, and then call [`QueryPipeline::next_batch`] again.
    MoreDataNeeded(Vec<DataRequest>),

    /// The pipeline has produced a batch of query results.
    ///
    /// The receiver should return these results to the user.
    Batch(Vec<T>),

    /// The pipeline has concluded processing and has no more results to produce.
    Done,
}

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
    pub fn new(plan: QueryPlan, pkranges: impl IntoIterator<Item = PartitionKeyRange>) -> Self {
        let merge_strategy = if plan.query_info.order_by.is_empty() {
            MergeStrategy::Unordered
        } else {
            MergeStrategy::Ordered(plan.query_info.order_by)
        };

        let producer = ItemProducer::new(pkranges, merge_strategy);

        let mut pipeline: Vec<Box<dyn PipelineNode<T>>> = Vec::new();

        if let Some(limit) = plan.query_info.limit {
            pipeline.push(Box::new(LimitPipelineNode::new(limit)));
        }

        Self {
            pipeline,
            producer,
            terminated: false,
        }
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
    pub fn next_batch(&mut self) -> crate::Result<PipelineResponse<T>> {
        if self.terminated {
            return Ok(PipelineResponse::Done);
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

        if batch.is_empty() {
            // If there are no items in the batch, we need to request more data.
            let requests = self.producer.data_requests();

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
    ) -> QueryResult<Item> {
        let item = Item::new(
            id.clone(),
            partition_id.to_string(),
            format!("{} / {}", partition_id, id),
        );
        let order_by_items = order_by_items
            .into_iter()
            .map(|value| serde_json::from_value(value).unwrap())
            .collect();
        QueryResult::new(vec![], order_by_items, item)
    }

    fn create_items(partition_id: &str, start_id: usize, count: usize) -> Vec<QueryResult<Item>> {
        (0..count)
            .map(|i| {
                let id = format!("item{}", start_id + i);
                create_item(partition_id, id, Vec::new())
            })
            .collect()
    }

    fn create_pipeline(query_info: Option<QueryInfo>) -> QueryPipeline<Item> {
        let plan = QueryPlan {
            partitioned_query_execution_info_version: 1,
            query_info: query_info.unwrap_or_default(),
            query_ranges: vec![],
        };
        QueryPipeline::new(
            plan,
            [
                PartitionKeyRange::new("partition0", "00", "99"),
                PartitionKeyRange::new("partition1", "99", "FF"),
            ],
        )
    }

    fn drain_pipeline(pipeline: &mut QueryPipeline<Item>) -> crate::Result<Vec<Item>> {
        Ok(assert_match!(pipeline.next_batch()?, PipelineResponse::Batch(e) => e))
    }

    // We do most of our testing here, because this is what the user will interact with.
    // The one exception is the merge strategies, which are tested in the producer module.

    #[test]
    pub fn next_batch_no_nodes() -> Result<(), Box<dyn std::error::Error>> {
        let mut pipeline = create_pipeline(None);

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

    #[test]
    pub fn next_batch_with_limit() -> Result<(), Box<dyn std::error::Error>> {
        let mut pipeline = create_pipeline(Some(QueryInfo {
            top: Some(7), // All items in p0, one item from p1
            ..Default::default()
        }));

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
