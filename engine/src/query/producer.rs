use std::{cmp::Ordering, collections::VecDeque, fmt::Debug};

use crate::{
    query::{DataRequest, PartitionKeyRange, QueryResult, SortOrder},
    ErrorKind,
};

use super::QueryClauseItem;

/// Represents the current stage that a partition is in during the query.
#[derive(Debug)]
enum PartitionStage {
    /// The partition is ready for the first data request. There should be no data in the queue yet.
    Initial,

    /// The partition has a pending continuation. When the current queue is exhausted, the continuation can be used to fetch more data.
    Continuing(String),

    /// The partition has been exhausted. When the current queue is exhausted, the partition is done.
    Done,
}

#[derive(Debug)]
pub enum MergeStrategy {
    Ordered(Vec<SortOrder>),
    Unordered,
}

struct PartitionState<T: Debug, I: QueryClauseItem> {
    pkrange: PartitionKeyRange,
    queue: VecDeque<QueryResult<T, I>>,
    stage: PartitionStage,
}

impl<T: Debug, I: QueryClauseItem> PartitionState<T, I> {
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

    #[tracing::instrument(level = "trace", skip_all, fields(pkrange_id = %self.pkrange.id))]
    pub fn extend(
        &mut self,
        item: impl IntoIterator<Item = QueryResult<T, I>>,
        continuation: Option<String>,
    ) {
        self.queue.extend(item);
        self.stage = continuation.map_or_else(
            || PartitionStage::Done,
            |token| PartitionStage::Continuing(token),
        );
        tracing::trace!(queue_len = self.queue.len(), stage = ?self.stage, "updated partition state");
    }

    #[tracing::instrument(level = "trace", skip_all, fields(pkrange_id = %self.pkrange.id))]
    pub fn next_data_request(&self) -> Option<DataRequest> {
        // If the queue is not empty, we don't need to request more data.
        if !self.queue.is_empty() {
            tracing::trace!("skipping data request for non-empty queue");
            return None;
        }

        match &self.stage {
            PartitionStage::Initial => {
                tracing::trace!("starting partition");
                Some(DataRequest {
                    pkrange_id: self.pkrange.id.clone().into(),
                    continuation: None,
                })
            }
            PartitionStage::Continuing(token) => {
                tracing::trace!(continuation_token = ?token, "continuing partition");
                Some(DataRequest {
                    pkrange_id: self.pkrange.id.clone().into(),
                    continuation: Some(token.clone()),
                })
            }
            PartitionStage::Done => {
                tracing::trace!("partition exhausted");
                None
            }
        }
    }

    pub fn has_started(&self) -> bool {
        !matches!(self.stage, PartitionStage::Initial)
    }
}

pub struct ItemProducer<T: Debug, I: QueryClauseItem> {
    partitions: Vec<PartitionState<T, I>>,
    strategy: MergeStrategy,
}

impl<T: Debug, I: QueryClauseItem> ItemProducer<T, I> {
    pub fn new(
        pkranges: impl IntoIterator<Item = PartitionKeyRange>,
        strategy: MergeStrategy,
    ) -> Self {
        let partitions = pkranges
            .into_iter()
            .map(|r| PartitionState {
                pkrange: r,
                queue: VecDeque::new(),
                stage: PartitionStage::Initial,
            })
            .collect();
        Self {
            partitions,
            strategy,
        }
    }

    pub fn data_requests(&self) -> Vec<DataRequest> {
        self.partitions
            .iter()
            .filter_map(|p| p.next_data_request())
            .collect()
    }

    #[tracing::instrument(level = "trace", skip_all, fields(pkrange_id = %pkrange_id, item_count = data.len(), continuation = ?continuation))]
    pub fn provide_data(
        &mut self,
        pkrange_id: &str,
        data: Vec<QueryResult<T, I>>,
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

    #[tracing::instrument(level = "trace", skip(self))]
    pub fn produce_item(&mut self) -> crate::Result<Option<QueryResult<T, I>>> {
        let mut next_partition = None;
        for partition in &mut self.partitions {
            let _ = tracing::trace_span!("produce_item::check_partition", pkrange_id = %partition.pkrange.id)
                .entered();

            // If any partition hasn't started, we can't return any items.
            if !partition.has_started() {
                tracing::trace!(pkrange_id = ?partition.pkrange.id, "found partition that hasn't started yet, returning no item");
                return Ok(None);
            }

            if partition.exhausted() {
                tracing::trace!(pkrange_id = ?partition.pkrange.id, "skipping exhausted partition");
                continue;
            }

            next_partition = match (next_partition, partition) {
                (None, p) => Some(p),
                (Some(left), right) => {
                    // Take the "smaller" partition.
                    if compare_partitions(&self.strategy, left, right)? == Ordering::Greater {
                        tracing::trace!(left = ?left.pkrange.id, right = ?right.pkrange.id, "right partition sorts earlier");
                        Some(right)
                    } else {
                        tracing::trace!(left = ?left.pkrange.id, right = ?right.pkrange.id, "left partition sorts earlier");
                        Some(left)
                    }
                }
            }
        }

        Ok(next_partition.and_then(|p| p.queue.pop_front()))
    }
}

#[tracing::instrument(level = "trace", skip(left, right), fields(left_pkrange_id = %left.pkrange.id, right_pkrange_id = %right.pkrange.id))]
fn compare_partitions<T: Debug, I: QueryClauseItem>(
    strategy: &MergeStrategy,
    left: &PartitionState<T, I>,
    right: &PartitionState<T, I>,
) -> crate::Result<Ordering> {
    match strategy {
        MergeStrategy::Unordered => {
            tracing::trace!(left_min = ?left.pkrange.min_inclusive, right_min = ?right.pkrange.min_inclusive, "comparing partitions");
            Ok(left.pkrange.min_inclusive.cmp(&right.pkrange.min_inclusive))
        }
        MergeStrategy::Ordered(orderings) => {
            let (left_item, right_item) = match (left.queue.front(), right.queue.front()) {
                (Some(left), Some(right)) => (left, right),

                // "Empty" partitions sort before non-empty partitions, because they need to cause iteration to stop so we can get more data.
                (None, Some(_)) => return Ok(Ordering::Less),
                (Some(_), None) => return Ok(Ordering::Greater),
                (None, None) => {
                    return Ok(left.pkrange.min_inclusive.cmp(&right.pkrange.min_inclusive))
                }
            };
            tracing::trace!(?left_item, ?right_item, "comparing items");
            match left_item.compare(right_item, orderings)? {
                Ordering::Equal => Ok(left.pkrange.min_inclusive.cmp(&right.pkrange.min_inclusive)),
                order => Ok(order),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};
    use serde_json::json;

    use crate::query::{query_result::JsonQueryClauseItem, PartitionKeyRange, QueryResult};

    use super::*;

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
        pkrange_id: &str,
        id: impl Into<String>,
        order_by_items: Vec<serde_json::Value>,
    ) -> QueryResult<Item, JsonQueryClauseItem> {
        let id = id.into();
        let item = Item::new(
            id.clone(),
            pkrange_id.to_string(),
            format!("{} / {}", pkrange_id, id),
        );
        let order_by_items = order_by_items
            .into_iter()
            .map(|value| serde_json::from_value(value).unwrap())
            .collect();
        QueryResult::new(vec![], order_by_items, item)
    }

    fn drain_producer<T: Debug, I: QueryClauseItem>(
        producer: &mut ItemProducer<T, I>,
    ) -> crate::Result<Vec<T>> {
        let mut items = Vec::new();
        while let Some(item) = producer.produce_item()? {
            let item = item.into_payload();
            items.push(item);
        }
        Ok(items)
    }

    #[test]
    pub fn unordered_strategy_orders_by_partition_key_minimum(
    ) -> Result<(), Box<dyn std::error::Error>> {
        fn fill_partition(
            producer: &mut ItemProducer<Item, JsonQueryClauseItem>,
            pkrange_id: &str,
            start_id: usize,
            count: usize,
            continuation: Option<String>,
        ) -> crate::Result<()> {
            // NOTE: A PKRange ID is NOT the same as a partition key, but in our testing it can serve that purpose.

            let mut items = Vec::new();
            for i in 0..count {
                let id = format!("item{}", start_id + i);
                items.push(create_item(pkrange_id, id, Vec::new()));
            }

            producer.provide_data(pkrange_id, items, continuation)
        }

        let mut producer = ItemProducer::new(
            vec![
                PartitionKeyRange::new("partition0", "00", "99"),
                PartitionKeyRange::new("partition1", "99", "FF"),
            ],
            MergeStrategy::Unordered,
        );
        fill_partition(&mut producer, "partition0", 0, 5, Some("p0c0".to_string()))?;
        fill_partition(&mut producer, "partition1", 0, 5, Some("p1c0".to_string()))?;

        let items = drain_producer(&mut producer)?;

        assert_eq!(
            vec![
                Item::new("item0", "partition0", "partition0 / item0"),
                Item::new("item1", "partition0", "partition0 / item1"),
                Item::new("item2", "partition0", "partition0 / item2"),
                Item::new("item3", "partition0", "partition0 / item3"),
                Item::new("item4", "partition0", "partition0 / item4"),
            ],
            items
        );

        // Now, refill partition0 and continue iterating, but mark it done (no further continuations).
        fill_partition(&mut producer, "partition0", 5, 5, None)?;

        // We should see the merge strategy move to partition1 after exhausting partition0.
        let items = drain_producer(&mut producer)?;
        assert_eq!(
            vec![
                Item::new("item5", "partition0", "partition0 / item5"),
                Item::new("item6", "partition0", "partition0 / item6"),
                Item::new("item7", "partition0", "partition0 / item7"),
                Item::new("item8", "partition0", "partition0 / item8"),
                Item::new("item9", "partition0", "partition0 / item9"),
                Item::new("item0", "partition1", "partition1 / item0"),
                Item::new("item1", "partition1", "partition1 / item1"),
                Item::new("item2", "partition1", "partition1 / item2"),
                Item::new("item3", "partition1", "partition1 / item3"),
                Item::new("item4", "partition1", "partition1 / item4"),
            ],
            items
        );

        // Now, finally, refill partition1 and continue iterating, but mark it done (no further continuations).
        fill_partition(&mut producer, "partition1", 5, 5, None)?;

        // And we should get the remaining items from partition1.
        let items = drain_producer(&mut producer)?;
        assert_eq!(
            vec![
                Item::new("item5", "partition1", "partition1 / item5"),
                Item::new("item6", "partition1", "partition1 / item6"),
                Item::new("item7", "partition1", "partition1 / item7"),
                Item::new("item8", "partition1", "partition1 / item8"),
                Item::new("item9", "partition1", "partition1 / item9"),
            ],
            items
        );

        Ok(())
    }

    #[test]
    pub fn ordered_strategy_orders_by_order_by_items_sorted_in_specified_orders(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut producer = ItemProducer::new(
            vec![
                PartitionKeyRange::new("partition0", "00", "99"),
                PartitionKeyRange::new("partition1", "99", "FF"),
            ],
            MergeStrategy::Ordered(vec![SortOrder::Ascending, SortOrder::Descending]),
        );

        let p0items = vec![
            create_item(
                &producer.partitions[0].pkrange.id,
                "item0",
                vec![json!({"item": 1}), json!({"item": "aaaa"})],
            ),
            create_item(
                &producer.partitions[0].pkrange.id,
                "item1",
                vec![json!({"item": 2}), json!({"item": "yyyy"})],
            ),
            create_item(
                &producer.partitions[0].pkrange.id,
                "item2",
                vec![json!({"item": 6}), json!({"item": "zzzz"})],
            ),
        ];

        let p1items = vec![
            create_item(
                &producer.partitions[1].pkrange.id,
                "item0",
                vec![json!({"item": 1}), json!({"item": "zzzz"})],
            ),
            create_item(
                &producer.partitions[1].pkrange.id,
                "item1",
                vec![json!({"item": 2}), json!({"item": "bbbb"})],
            ),
            create_item(
                &producer.partitions[1].pkrange.id,
                "item2",
                vec![json!({"item": 3}), json!({"item": "zzzz"})],
            ),
            create_item(
                &producer.partitions[1].pkrange.id,
                "item3",
                vec![json!({"item": 7}), json!({"item": "zzzz"})],
            ),
            create_item(
                &producer.partitions[1].pkrange.id,
                "item4",
                vec![json!({"item": 8}), json!({"item": "zzzz"})],
            ),
            create_item(
                &producer.partitions[1].pkrange.id,
                "item5",
                vec![json!({"item": 9}), json!({"item": "zzzz"})],
            ),
        ];

        // Set both partitions as "continuing".
        producer.provide_data("partition0", p0items, Some("p0c0".to_string()))?;
        producer.provide_data("partition1", p1items, Some("p1c0".to_string()))?;

        // We should stop once any partition's queue is empty.
        let items = drain_producer(&mut producer)?;
        assert_eq!(
            vec![
                Item::new("item0", "partition1", "partition1 / item0"),
                Item::new("item0", "partition0", "partition0 / item0"),
                Item::new("item1", "partition0", "partition0 / item1"),
                Item::new("item1", "partition1", "partition1 / item1"),
                Item::new("item2", "partition1", "partition1 / item2"),
                Item::new("item2", "partition0", "partition0 / item2"),
            ],
            items
        );

        // Mark partition 0 as done, with no additional data provided
        producer.provide_data("partition0", vec![], None)?;

        // We should get the rest of partition1's items.
        let items = drain_producer(&mut producer)?;
        assert_eq!(
            vec![
                Item::new("item3", "partition1", "partition1 / item3"),
                Item::new("item4", "partition1", "partition1 / item4"),
                Item::new("item5", "partition1", "partition1 / item5"),
            ],
            items
        );

        // Queue more items in partition0
        Ok(())
    }
}
