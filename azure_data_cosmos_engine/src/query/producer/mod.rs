// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use std::{
    cmp::Ordering,
    collections::{BinaryHeap, VecDeque},
    fmt::Debug,
};

use crate::{
    query::{
        producer::{
            sorting::{SortableResult, Sorting},
            state::PartitionState,
        },
        DataRequest, PartitionKeyRange, QueryResult, SortOrder,
    },
    ErrorKind,
};

use super::QueryClauseItem;

mod sorting;
mod state;

/// Indicates the way in which multiple partition results should be merged.
enum ProducerStrategy<T: Debug, I: QueryClauseItem> {
    /// Results are not re-ordered by the query and should be ordered by the partition key range minimum.
    Unordered {
        current_partition_index: usize,
        current_pkrange_id: Option<String>,
        items: VecDeque<QueryResult<T, I>>,
    },

    /// Results should be merged by comparing the sort order of the `ORDER BY` items. Results can be streamed, because each partition will provide data in a global order.
    Streaming {
        sorting: Sorting,
        buffers: Vec<(String, VecDeque<QueryResult<T, I>>)>, // (partition key range ID, buffer)
    },
    /// Results should be merged by comparing the sort order of the `ORDER BY` items. Results cannot be streamed, because each partition will provide data in a local order.
    NonStreaming {
        sorting: Sorting,
        items: BinaryHeap<SortableResult<T, I>>,
    },
}

impl<T: Debug, I: QueryClauseItem> ProducerStrategy<T, I> {
    pub fn requests(&mut self, partitions: &[PartitionState]) -> Option<Vec<DataRequest>> {
        // Fetches the next set of requests to be made to get additional data.
        match self {
            ProducerStrategy::Unordered {
                ref mut current_partition_index,
                ref mut current_pkrange_id,
                ..
            } => {
                // In the unordered strategy, we simply return the first partition key range's request.
                // Once that partition is exhausted, we remove it from the list and return the next one.
                let mut requests = Vec::new();
                while requests.is_empty() {
                    // If there are no more partitions, return None.
                    let partition = partitions.get(*current_partition_index)?;
                    match partition.request() {
                        Some(request) => {
                            tracing::trace!(pkrange_id = ?partition.pkrange.id, "requesting data for partition");
                            requests.push(request);
                        }
                        None => {
                            tracing::trace!(pkrange_id = ?partition.pkrange.id, "partition exhausted, removing from list");
                            *current_partition_index += 1;
                            *current_pkrange_id = partitions
                                .get(*current_partition_index)
                                .map(|p| p.pkrange.id.clone());
                        }
                    }
                }
                Some(requests)
            }

            // In the ordered strategies, we return a request for each partition.
            ProducerStrategy::Streaming { .. } | ProducerStrategy::NonStreaming { .. } => {
                let requests = partitions
                    .iter()
                    .filter_map(|partition| partition.request())
                    .collect::<Vec<_>>();
                // If there are no requests, we return None.
                if requests.is_empty() {
                    None
                } else {
                    Some(requests)
                }
            }
        }
    }

    pub fn provide_data(
        &mut self,
        partition: &PartitionState,
        data: Vec<QueryResult<T, I>>,
    ) -> crate::Result<()> {
        // Provides data for the given partition key range.
        match self {
            ProducerStrategy::Unordered {
                current_pkrange_id,
                items,
                ..
            } => {
                match current_pkrange_id {
                    Some(id) => {
                        if *id != partition.pkrange.id {
                            // The caller provided data for a different partition key range ID before draining the current items queue.
                            return Err(ErrorKind::InternalError.with_message(format!(
                                    "provided data for partition key range ID: {}, but current partition is: {}",
                                    partition.pkrange.id, id
                                )));
                        }
                    }
                    None => {
                        return Err(ErrorKind::InternalError.with_message(format!(
                            "provided data for partition key range ID: {}, but all partitions are exhausted",
                            partition.pkrange.id
                        )));
                    }
                }

                // Add the data to the items queue. There's no ordering to worry about, so we just append the items.
                items.extend(data);
            }
            ProducerStrategy::Streaming { buffers, .. } => {
                // Find the buffer for the given partition key range ID.
                let (pkrange_id, buffer) = buffers.get_mut(partition.index).ok_or_else(|| {
                    ErrorKind::InternalError.with_message(format!(
                        "missing buffer for partition key range ID: {}",
                        partition.pkrange.id
                    ))
                })?;
                debug_assert_eq!(
                    pkrange_id, &partition.pkrange.id,
                    "buffer ID should match partition key range ID",
                );
                // We assume the data is coming from the server pre-sorted, so we can just extend the buffer with the data.
                buffer.extend(data);
            }
            ProducerStrategy::NonStreaming { sorting, items } => {
                // Insert the items into the heap as we go, which will keep them sorted
                for item in data {
                    // We need to sort the items by the order by items, so we create a SortableResult.
                    items.push(SortableResult::new(sorting.clone(), item));
                }
            }
        }
        Ok(())
    }

    pub fn produce_item(
        &mut self,
        partitions: &[PartitionState],
    ) -> crate::Result<Option<QueryResult<T, I>>> {
        // Gets the next item from the merge strategy.
        match self {
            ProducerStrategy::Unordered { items, .. } => Ok(items.pop_front()),
            ProducerStrategy::Streaming { sorting, buffers } => {
                // Scan through each partition to find the next item to produce.
                // We do the scan first with an immutable borrow of the buffers, and then end up with the index of the partition that has the next item to produce.
                // Then we can borrow the buffer mutably after the loop to pop the item out of it.
                let mut current_match = None;
                for (i, partition) in partitions.iter().enumerate() {
                    let (pkrange_id, buffer) = buffers.get(i).ok_or_else(|| {
                        ErrorKind::InternalError.with_message(format!(
                            "missing buffer for partition key range ID: {}",
                            partition.pkrange.id
                        ))
                    })?;
                    debug_assert_eq!(pkrange_id, &partition.pkrange.id); // This should always be true, as the lists are initialized together.

                    if !partition.started() {
                        // If any partition hasn't started, we have to stop producing items.
                        // A Partition is considered "started" when we've received at least one `provide_data` call referencing it.
                        // For a streaming order by, we can't stream ANY results until we've received at least one set of results from each partition.
                        // The missing partitions may contain values that sort BEFORE items in the partitions we've received.
                        //
                        // SDKs could optimize how they call the engine to avoid this scenario (by always making requests first, for example),
                        // but we can't assume that will always be the case.
                        tracing::debug!(pkrange_id = ?partition.pkrange.id, "partition not started, stopping item production");
                        return Ok(None);
                    }

                    if partition.done() && buffer.is_empty() {
                        // If the partition is done and the buffer is empty, we can skip it.
                        // In fact, we NEED to skip it because we know it won't produce any more items and if we leave it in the set of partitions we consider,
                        // we might end up trying to query it for more data.
                        tracing::debug!(pkrange_id = ?partition.pkrange.id, "partition done and buffer empty, skipping");
                        continue;
                    }

                    match current_match {
                        None => {
                            // If we haven't found a match yet, we set the current match to this partition so that we always pick a partition.
                            current_match = Some((i, buffer.front()));
                        }
                        Some((current_index, current_item)) => {
                            match sorting.compare(
                                current_item.map(|r| r.order_by_items.as_slice()),
                                buffer.front().map(|r| r.order_by_items.as_slice()),
                            )? {
                                Ordering::Greater => {
                                    // The current item sorts higher than the new item, so we keep the current match.
                                    continue;
                                }
                                Ordering::Less => {
                                    // The new item sorts higher than the current item, so we update the current match to this partition.
                                    // Note: This might be because the partition's buffer is currently empty.
                                    // That can result in the selected partition being one with an empty buffer.
                                    // This is intentional, see below where we return the item.
                                    current_match = Some((i, buffer.front()));
                                }
                                Ordering::Equal => {
                                    // Compare the index of the partitions to ensure we always return the first partition with the same item.
                                    if i < current_index {
                                        // The new item is equal to the current item, but the partition index is lower,
                                        // so we update the current match to this partition.
                                        current_match = Some((i, buffer.front()));
                                    }
                                }
                            }
                        }
                    }
                }
                if let Some((i, _)) = current_match {
                    // We found a match, pop the item out of the buffer and return it.
                    debug_assert_eq!(
                        buffers[i].0, partitions[i].pkrange.id,
                        "buffer ID should match partition key range ID",
                    );
                    // If the buffer is empty, this may return `None`. That's by design!
                    // It means the partition has an empty buffer, and we may need to fetch more data for it.
                    // If it was fully exhausted, the check for `done() && buffer.is_empty()` would have excluded it.
                    // Instead, we have an empty buffer AND the possibility for more data from this partition.
                    // That means we WANT to return `None` here. We need to check this partition for more data before we can yield an item.
                    Ok(buffers[i].1.pop_front())
                } else {
                    // No match found, meaning all partitions are either exhausted or waiting for data.
                    Ok(None)
                }
            }
            ProducerStrategy::NonStreaming { items, .. } => {
                // We can only produce items when all partitions are done.
                if partitions.iter().any(|p| !p.done()) {
                    // If any partition is not done, we cannot produce items yet.
                    tracing::debug!("not all partitions are done, cannot produce items");
                    return Ok(None);
                }

                // We can just pop the next item from the heap, as it is already sorted.
                Ok(items.pop().map(|r| r.into()))
            }
        }
    }
}

/// An item producer handles merging results from several partitions into a single stream of results.
///
/// The single-partition result streams are merged according to a [`ProducerStrategy`] selected when the producer is initialized.
/// The producer is only responsible for handling ordering the results, other query operations like aggregations or offset/limit
/// are handled by the pipeline that runs after a specific item has been produced.
/// Ordering can't really be done by the pipeline though, since it may require buffering results from some or all partitions.
/// So, before the pipeline runs, the producer is responsible for actually organizing the initial set of results in the correct order.
pub struct ItemProducer<T: Debug, I: QueryClauseItem> {
    strategy: ProducerStrategy<T, I>,
    partitions: Vec<PartitionState>,
}

fn create_partition_state(
    pkranges: impl IntoIterator<Item = PartitionKeyRange>,
) -> Vec<PartitionState> {
    let mut partitions = pkranges
        .into_iter()
        .enumerate()
        .map(|(i, p)| PartitionState::new(i, p))
        .collect::<Vec<_>>();
    partitions.sort();
    partitions
}

impl<T: Debug, I: QueryClauseItem> ItemProducer<T, I> {
    /// Creates a producer for queries without ORDER BY clauses.
    ///
    /// This strategy processes partitions sequentially in partition key range order,
    /// exhausting one partition completely before moving to the next.
    ///
    /// Use this for queries that don't require global ordering across partitions.
    pub fn unordered(pkranges: impl IntoIterator<Item = PartitionKeyRange>) -> Self {
        let partitions = create_partition_state(pkranges);
        Self {
            strategy: ProducerStrategy::Unordered {
                current_partition_index: 0,
                current_pkrange_id: partitions.first().map(|p| p.pkrange.id.clone()),
                items: VecDeque::new(),
            },
            partitions,
        }
    }

    /// Creates a producer for ORDER BY queries where each partition returns globally sorted results.
    ///
    /// This strategy can stream results immediately because it assumes each partition's results
    /// are already sorted in the global order. It maintains a small buffer per partition and
    /// continuously merges the "head" items to produce the next globally ordered result.
    ///
    /// Use this when:
    /// - The query has an ORDER BY clause
    /// - Each partition's results are sorted in the same global order
    /// - You want to stream results without waiting for all partitions to complete
    pub fn streaming(
        pkranges: impl IntoIterator<Item = PartitionKeyRange>,
        sorting: Vec<SortOrder>,
    ) -> Self {
        let partitions = create_partition_state(pkranges);
        Self {
            strategy: ProducerStrategy::Streaming {
                sorting: Sorting::new(sorting),
                buffers: partitions
                    .iter()
                    .map(|p| (p.pkrange.id.clone(), VecDeque::new()))
                    .collect(),
            },
            partitions,
        }
    }

    /// Creates a producer for ORDER BY queries where partitions return locally sorted results.
    ///
    /// This strategy buffers ALL results from ALL partitions before returning any items.
    /// It uses a binary heap to sort results globally after collecting everything.
    /// No results can be streamed until all partitions are completely exhausted.
    ///
    /// Use this when:
    /// - The query has an ORDER BY clause
    /// - Each partition's results are only sorted locally (not in global order)
    /// - You can afford to buffer the entire result set in memory
    /// - Correctness is more important than streaming performance
    pub fn non_streaming(
        pkranges: impl IntoIterator<Item = PartitionKeyRange>,
        sorting: Vec<SortOrder>,
    ) -> Self {
        let partitions = create_partition_state(pkranges);
        Self {
            strategy: ProducerStrategy::NonStreaming {
                sorting: Sorting::new(sorting),
                items: BinaryHeap::new(),
            },
            partitions,
        }
    }

    /// Gets the [`DataRequest`]s that must be performed in order to add additional data to the partition buffers.
    pub fn data_requests(&mut self) -> Vec<DataRequest> {
        // The default value for Vec is an empty vec, which doesn't allocate until items are added.
        self.strategy.requests(&self.partitions).unwrap_or_default()
    }

    /// Provides additional data for the given partition.
    pub fn provide_data(
        &mut self,
        pkrange_id: &str,
        data: Vec<QueryResult<T, I>>,
        continuation: Option<String>,
    ) -> crate::Result<()> {
        let partition = self
            .partitions
            .iter_mut()
            .find(|p| p.pkrange.id == pkrange_id)
            .ok_or_else(|| {
                ErrorKind::UnknownPartitionKeyRange
                    .with_message(format!("unknown partition key range ID: {pkrange_id}"))
            })?;
        self.strategy.provide_data(partition, data)?;
        partition.update_state(continuation);

        Ok(())
    }

    /// Requests the next item from the cross-partition result stream.
    #[tracing::instrument(level = "trace", skip(self))]
    pub fn produce_item(&mut self) -> crate::Result<Option<QueryResult<T, I>>> {
        self.strategy.produce_item(&self.partitions)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

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

    pub type TestPage = (Option<String>, Vec<QueryResult<Item, JsonQueryClauseItem>>);

    fn create_item(
        pkrange_id: &str,
        id: impl Into<String>,
        order_by_items: Vec<serde_json::Value>,
    ) -> QueryResult<Item, JsonQueryClauseItem> {
        let id = id.into();
        let item = Item::new(
            id.clone(),
            pkrange_id.to_string(),
            format!("{pkrange_id} / {id}"),
        );
        let order_by_items = order_by_items
            .into_iter()
            .map(|value| serde_json::from_value(value).unwrap())
            .collect();
        QueryResult::new(vec![], order_by_items, item)
    }

    fn run_producer(
        producer: &mut ItemProducer<Item, JsonQueryClauseItem>,
        mut partitions: HashMap<String, VecDeque<TestPage>>,
    ) -> crate::Result<Vec<Item>> {
        let mut items = Vec::new();
        loop {
            let requests = producer.data_requests();
            if requests.is_empty() {
                // No more requests, we can stop.
                return Ok(items);
            }
            for request in requests {
                let pkrange_id = request.pkrange_id.to_string();
                if let Some(pages) = partitions.get_mut(&pkrange_id) {
                    let (token, items) = pages.pop_front().unwrap_or_else(|| (None, Vec::new()));
                    assert_eq!(
                        request.continuation, token,
                        "continuation token should match the one provided in the request"
                    );
                    let next_token = pages.front().and_then(|(t, _)| t.clone());
                    producer.provide_data(&pkrange_id, items, next_token)?;
                } else {
                    return Err(ErrorKind::UnknownPartitionKeyRange
                        .with_message(format!("unknown partition key range ID: {pkrange_id}")));
                }
            }

            // Now drain the items from the producer.
            while let Some(item) = producer.produce_item()? {
                items.push(item.into_payload());
            }

            // Loop back around to check for requests.
        }
    }

    #[test]
    pub fn unordered_strategy_orders_by_partition_key_minimum(
    ) -> Result<(), Box<dyn std::error::Error>> {
        // The partitions are "Vec<(Option<String>, Vec<Item>)>", where the first element is the continuation token
        // and the second element is the list of items for that partition.
        let mut partition0: VecDeque<TestPage> = VecDeque::new();
        let mut partition1: VecDeque<TestPage> = VecDeque::new();

        /// Generates a page of test items for a given partition.
        fn fill_page(
            partition: &mut VecDeque<TestPage>,
            pkrange_id: &str,
            start_id: usize,
            count: usize,
            continuation: Option<String>,
        ) -> crate::Result<()> {
            // NOTE: A PKRange ID is NOT the same as a partition key, but in our testing it can serve that purpose.

            let mut page = Vec::new();
            for i in 0..count {
                let id = format!("item{}", start_id + i);
                page.push(create_item(pkrange_id, id, Vec::new()));
            }

            partition.push_back((continuation, page));
            Ok(())
        }

        // Two pages of 5 items for each partition
        fill_page(&mut partition0, "partition0", 0, 5, None)?;
        fill_page(
            &mut partition0,
            "partition0",
            5,
            5,
            Some("p0c0".to_string()),
        )?;
        fill_page(&mut partition1, "partition1", 0, 5, None)?;
        fill_page(
            &mut partition1,
            "partition1",
            5,
            5,
            Some("p1c0".to_string()),
        )?;

        let mut producer = ItemProducer::unordered(vec![
            PartitionKeyRange::new("partition0", "00", "99"),
            PartitionKeyRange::new("partition1", "99", "FF"),
        ]);

        let items = run_producer(
            &mut producer,
            HashMap::from([
                ("partition0".to_string(), partition0),
                ("partition1".to_string(), partition1),
            ]),
        )?;

        assert_eq!(
            vec![
                Item::new("item0", "partition0", "partition0 / item0"),
                Item::new("item1", "partition0", "partition0 / item1"),
                Item::new("item2", "partition0", "partition0 / item2"),
                Item::new("item3", "partition0", "partition0 / item3"),
                Item::new("item4", "partition0", "partition0 / item4"),
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
    pub fn streaming_strategy_merges_ordered_streams_of_data(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut partition0: VecDeque<TestPage> = VecDeque::new();
        let mut partition1: VecDeque<TestPage> = VecDeque::new();

        // Partition 0 has two pages, but the second is empty (this can happen in real scenarios).
        partition0.push_back((
            None,
            vec![
                create_item(
                    "partition0",
                    "item0",
                    vec![json!({"item": 1}), json!({"item": "aaaa"})],
                ),
                create_item(
                    "partition0",
                    "item1",
                    vec![json!({"item": 2}), json!({"item": "yyyy"})],
                ),
                create_item(
                    "partition0",
                    "item2",
                    vec![json!({"item": 6}), json!({"item": "zzzz"})],
                ),
            ],
        ));
        partition0.push_back((Some("p0c0".to_string()), vec![]));

        // Partition 1 doesn't have a second page, so it will be exhausted after the first page.
        partition1.push_back((
            None,
            vec![
                create_item(
                    "partition1",
                    "item0",
                    vec![json!({"item": 1}), json!({"item": "zzzz"})],
                ),
                create_item(
                    "partition1",
                    "item1",
                    vec![json!({"item": 2}), json!({"item": "bbbb"})],
                ),
                create_item(
                    "partition1",
                    "item2",
                    vec![json!({"item": 3}), json!({"item": "zzzz"})],
                ),
                create_item(
                    "partition1",
                    "item3",
                    vec![json!({"item": 7}), json!({"item": "zzzz"})],
                ),
                create_item(
                    "partition1",
                    "item4",
                    vec![json!({"item": 8}), json!({"item": "zzzz"})],
                ),
                create_item(
                    "partition1",
                    "item5",
                    vec![json!({"item": 9}), json!({"item": "zzzz"})],
                ),
            ],
        ));

        let mut producer = ItemProducer::streaming(
            vec![
                PartitionKeyRange::new("partition0", "00", "99"),
                PartitionKeyRange::new("partition1", "99", "FF"),
            ],
            vec![SortOrder::Ascending, SortOrder::Descending],
        );

        // We should stop once any partition's queue is empty.
        let items = run_producer(
            &mut producer,
            HashMap::from([
                ("partition0".to_string(), partition0),
                ("partition1".to_string(), partition1),
            ]),
        )?;

        assert_eq!(
            vec![
                Item::new("item0", "partition1", "partition1 / item0"),
                Item::new("item0", "partition0", "partition0 / item0"),
                Item::new("item1", "partition0", "partition0 / item1"),
                Item::new("item1", "partition1", "partition1 / item1"),
                Item::new("item2", "partition1", "partition1 / item2"),
                Item::new("item2", "partition0", "partition0 / item2"),
                Item::new("item3", "partition1", "partition1 / item3"),
                Item::new("item4", "partition1", "partition1 / item4"),
                Item::new("item5", "partition1", "partition1 / item5"),
            ],
            items
        );

        Ok(())
    }

    #[test]
    pub fn nonstreaming_strategy_buffers_all_results_before_ordering(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut partition0: VecDeque<TestPage> = VecDeque::new();
        let mut partition1: VecDeque<TestPage> = VecDeque::new();

        // For this test, we basically use the same data as the streaming strategy, but each partition's results are not pre-sorted, in fact they're reverse-sorted.

        // Partition 0 has two pages, but the second is empty (this can happen in real scenarios).
        partition0.push_back((
            None,
            vec![
                create_item(
                    "partition0",
                    "item2",
                    vec![json!({"item": 6}), json!({"item": "zzzz"})],
                ),
                create_item(
                    "partition0",
                    "item1",
                    vec![json!({"item": 2}), json!({"item": "yyyy"})],
                ),
                create_item(
                    "partition0",
                    "item0",
                    vec![json!({"item": 1}), json!({"item": "aaaa"})],
                ),
            ],
        ));
        partition0.push_back((Some("p0c0".to_string()), vec![]));

        // Partition 1 doesn't have a second page, so it will be exhausted after the first page.
        partition1.push_back((
            None,
            vec![
                create_item(
                    "partition1",
                    "item5",
                    vec![json!({"item": 9}), json!({"item": "zzzz"})],
                ),
                create_item(
                    "partition1",
                    "item4",
                    vec![json!({"item": 8}), json!({"item": "zzzz"})],
                ),
                create_item(
                    "partition1",
                    "item3",
                    vec![json!({"item": 7}), json!({"item": "zzzz"})],
                ),
                create_item(
                    "partition1",
                    "item2",
                    vec![json!({"item": 3}), json!({"item": "zzzz"})],
                ),
                create_item(
                    "partition1",
                    "item1",
                    vec![json!({"item": 2}), json!({"item": "bbbb"})],
                ),
                create_item(
                    "partition1",
                    "item0",
                    vec![json!({"item": 1}), json!({"item": "zzzz"})],
                ),
            ],
        ));

        let mut producer = ItemProducer::non_streaming(
            vec![
                PartitionKeyRange::new("partition0", "00", "99"),
                PartitionKeyRange::new("partition1", "99", "FF"),
            ],
            vec![SortOrder::Ascending, SortOrder::Descending],
        );

        // We should stop once any partition's queue is empty.
        let items = run_producer(
            &mut producer,
            HashMap::from([
                ("partition0".to_string(), partition0),
                ("partition1".to_string(), partition1),
            ]),
        )?;

        assert_eq!(
            vec![
                Item::new("item0", "partition1", "partition1 / item0"),
                Item::new("item0", "partition0", "partition0 / item0"),
                Item::new("item1", "partition0", "partition0 / item1"),
                Item::new("item1", "partition1", "partition1 / item1"),
                Item::new("item2", "partition1", "partition1 / item2"),
                Item::new("item2", "partition0", "partition0 / item2"),
                Item::new("item3", "partition1", "partition1 / item3"),
                Item::new("item4", "partition1", "partition1 / item4"),
                Item::new("item5", "partition1", "partition1 / item5"),
            ],
            items
        );

        Ok(())
    }
}
