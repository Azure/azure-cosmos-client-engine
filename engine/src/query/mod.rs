use std::{cmp::Ordering, collections::VecDeque};

use serde::Deserialize;

use crate::{ErrorKind, Result};

mod plan;
mod value;

pub use plan::{QueryInfo, QueryPlan, QueryRange, SortOrder};
pub use value::{QueryPayload, QueryValue};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartitionKeyRange {
    id: String,
    min_inclusive: String,
    max_exclusive: String,
}

struct PartitionState<P> {
    pkrange: PartitionKeyRange,
    queue: VecDeque<P>,
}

enum MergeStrategy {
    Ordered(Vec<SortOrder>),
    Unordered,
}

fn compare_partitions_by_items<P: QueryPayload>(
    orderings: &[SortOrder],
    left: &PartitionState<P>,
    right: &PartitionState<P>,
) -> Result<Ordering> {
    let (mut left_values, mut right_values) = match (left.queue.front(), right.queue.front()) {
        (Some(left), Some(right)) => (left.iter_order_by_values(), right.iter_order_by_values()),
        (None, Some(_)) => return Ok(Ordering::Less),
        (Some(_), None) => return Ok(Ordering::Greater),
        (None, None) => return Ok(Ordering::Equal),
    };
    for ordering in orderings {
        let (Some(left), Some(right)) = (left_values.next(), right_values.next()) else {
            return Err(ErrorKind::QueryPlanInvalid
                .with_message("items have inconsistent numbers of order by items"));
        };
        let order = left.cmp(&right);
        let order = match ordering {
            SortOrder::Ascending => order,
            SortOrder::Descending => order.reverse(),
        };
        if order != Ordering::Equal {
            // The values are different, so we can return the order
            return Ok(order);
        }
    }

    // The values have been equal so far, so compare the partition keys
    Ok(left.pkrange.min_inclusive.cmp(&right.pkrange.min_inclusive))
}

impl MergeStrategy {
    pub fn next_partition<'a, P: QueryPayload>(
        &self,
        partitions: &'a mut [PartitionState<P>],
    ) -> Result<Option<&'a mut PartitionState<P>>> {
        let mut next_partition = None;
        for partition in partitions {
            if partition.queue.is_empty() {
                continue;
            }

            next_partition = match (next_partition, partition) {
                (None, p) => Some(p),
                (Some(left), right) => {
                    if self.compare_partitions(left, right)? == Ordering::Greater {
                        Some(right)
                    } else {
                        Some(left)
                    }
                }
            }
        }

        Ok(next_partition)
    }

    fn compare_partitions<P: QueryPayload>(
        &self,
        left: &PartitionState<P>,
        right: &PartitionState<P>,
    ) -> Result<Ordering> {
        match self {
            MergeStrategy::Unordered => {
                Ok(left.pkrange.min_inclusive.cmp(&right.pkrange.min_inclusive))
            }
            MergeStrategy::Ordered(ordering) => compare_partitions_by_items(&ordering, left, right),
        }
    }
}

pub struct QueryPipeline<P> {
    partitions: Vec<PartitionState<P>>,
    merge_strategy: MergeStrategy,
}

impl<P> QueryPipeline<P> {
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
}

impl<P: QueryPayload> QueryPipeline<P> {
    pub fn next_item(&mut self) -> Result<Option<P>> {
        let next_partition = self.merge_strategy.next_partition(&mut self.partitions)?;
        let next_item = next_partition.and_then(|p| p.queue.pop_front());
        Ok(next_item)
    }
}
