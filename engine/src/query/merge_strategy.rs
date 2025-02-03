use std::cmp::Ordering;

use super::{PartitionState, QueryResult, SortOrder};

pub enum MergeStrategy {
    Ordered(Vec<SortOrder>),
    Unordered,
}

impl MergeStrategy {
    pub fn item_iter<'a>(
        &'a self,
        partitions: &'a mut [PartitionState],
    ) -> impl Iterator<Item = crate::Result<QueryResult>> + 'a {
        ItemIter {
            partitions,
            strategy: self,
        }
    }

    pub fn next_item(
        &self,
        partitions: &mut [PartitionState],
    ) -> crate::Result<Option<QueryResult>> {
        let mut next_partition = None;
        for partition in partitions {
            // If any partition hasn't started, we can't return any items.
            if !partition.has_started() {
                return Ok(None);
            }

            if partition.exhausted() {
                continue;
            }

            next_partition = match (next_partition, partition) {
                (None, p) => Some(p),
                (Some(left), right) => {
                    // Take the "smaller" partition.
                    if self.compare_partitions(left, right)? == Ordering::Greater {
                        Some(right)
                    } else {
                        Some(left)
                    }
                }
            }
        }

        Ok(next_partition.and_then(|p| p.queue.pop_front()))
    }

    fn compare_partitions(
        &self,
        left: &PartitionState,
        right: &PartitionState,
    ) -> crate::Result<Ordering> {
        match self {
            MergeStrategy::Unordered => {
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
                match left_item.compare(right_item, orderings)? {
                    Ordering::Equal => {
                        Ok(left.pkrange.min_inclusive.cmp(&right.pkrange.min_inclusive))
                    }
                    order => Ok(order),
                }
            }
        }
    }
}

pub struct ItemIter<'a> {
    partitions: &'a mut [PartitionState],
    strategy: &'a MergeStrategy,
}

impl<'a> Iterator for ItemIter<'a> {
    type Item = crate::Result<QueryResult>;

    fn next(&mut self) -> Option<Self::Item> {
        self.strategy.next_item(self.partitions).transpose()
    }
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};
    use serde_json::json;

    use crate::query::{PartitionKeyRange, PartitionState, QueryResult};

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
    ) -> QueryResult {
        let id = id.into();
        let item = Item::new(
            id.clone(),
            pkrange_id.to_string(),
            format!("{} / {}", pkrange_id, id),
        );
        let s = serde_json::to_string(&item).unwrap();
        let raw = serde_json::value::RawValue::from_string(s).unwrap();
        let order_by_items = order_by_items
            .into_iter()
            .map(|value| serde_json::from_value(value).unwrap())
            .collect();
        QueryResult::new(vec![], order_by_items, raw)
    }

    fn drain_partitions<'a>(
        strategy: &'a MergeStrategy,
        partitions: &'a mut [PartitionState],
    ) -> crate::Result<Vec<Item>> {
        strategy
            .item_iter(partitions)
            .map(|item| item.and_then(|item| item.payload_into()))
            .collect::<Result<Vec<_>, _>>()
    }

    #[test]
    pub fn unordered_strategy_orders_by_partition_key_minimum(
    ) -> Result<(), Box<dyn std::error::Error>> {
        fn fill_partition(
            partition: &mut PartitionState,
            start_id: usize,
            count: usize,
            continuation: Option<String>,
        ) {
            // NOTE: A PKRange ID is NOT the same as a partition key, but in our testing it can serve that purpose.

            let mut items = Vec::new();
            for i in 0..count {
                let id = format!("item{}", start_id + i);
                items.push(create_item(&partition.pkrange.id, id, Vec::new()));
            }

            partition.extend(items, continuation);
        }

        let mut partitions = vec![
            PartitionState::new(PartitionKeyRange::new("partition0", "00", "99")),
            PartitionState::new(PartitionKeyRange::new("partition1", "99", "FF")),
        ];

        fill_partition(&mut partitions[0], 0, 5, Some("p0c0".to_string()));
        fill_partition(&mut partitions[1], 0, 5, Some("p1c0".to_string()));

        let strategy = MergeStrategy::Unordered;
        let items = drain_partitions(&strategy, &mut partitions)?;

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
        fill_partition(&mut partitions[0], 5, 5, None);

        // We should see the merge strategy move to partition1 after exhausting partition0.
        let items = drain_partitions(&strategy, &mut partitions)?;
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
        fill_partition(&mut partitions[1], 5, 5, None);

        // And we should get the remaining items from partition1.
        let items = drain_partitions(&strategy, &mut partitions)?;
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
        let strategy = MergeStrategy::Ordered(vec![SortOrder::Ascending, SortOrder::Descending]);
        let mut partitions = vec![
            PartitionState::new(PartitionKeyRange::new("partition0", "00", "99")),
            PartitionState::new(PartitionKeyRange::new("partition1", "99", "FF")),
        ];

        let p0items = vec![
            create_item(
                &partitions[0].pkrange.id,
                "item0",
                vec![json!({"item": 1}), json!({"item": "aaaa"})],
            ),
            create_item(
                &partitions[0].pkrange.id,
                "item1",
                vec![json!({"item": 2}), json!({"item": "yyyy"})],
            ),
            create_item(
                &partitions[0].pkrange.id,
                "item2",
                vec![json!({"item": 6}), json!({"item": "zzzz"})],
            ),
        ];

        let p1items = vec![
            create_item(
                &partitions[1].pkrange.id,
                "item0",
                vec![json!({"item": 1}), json!({"item": "zzzz"})],
            ),
            create_item(
                &partitions[1].pkrange.id,
                "item1",
                vec![json!({"item": 2}), json!({"item": "bbbb"})],
            ),
            create_item(
                &partitions[1].pkrange.id,
                "item2",
                vec![json!({"item": 3}), json!({"item": "zzzz"})],
            ),
            create_item(
                &partitions[1].pkrange.id,
                "item3",
                vec![json!({"item": 7}), json!({"item": "zzzz"})],
            ),
            create_item(
                &partitions[1].pkrange.id,
                "item4",
                vec![json!({"item": 8}), json!({"item": "zzzz"})],
            ),
            create_item(
                &partitions[1].pkrange.id,
                "item5",
                vec![json!({"item": 9}), json!({"item": "zzzz"})],
            ),
        ];

        // Set both partitions as "continuing".
        partitions[0].extend(p0items, Some("p0c0".to_string()));
        partitions[1].extend(p1items, Some("p1c0".to_string()));

        // We should stop once any partition's queue is empty.
        let items = drain_partitions(&strategy, &mut partitions)?;
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
        partitions[0].extend(vec![], None);

        // We should get the rest of partition1's items.
        let items = drain_partitions(&strategy, &mut partitions)?;
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
