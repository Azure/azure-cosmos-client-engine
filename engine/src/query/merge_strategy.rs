use std::cmp::Ordering;

use super::{PartitionState, SortOrder};
use crate::{ErrorKind, Result};

pub enum MergeStrategy {
    Ordered(Vec<SortOrder>),
    Unordered,
}

fn compare_partitions_by_items(
    orderings: &[SortOrder],
    left: &PartitionState,
    right: &PartitionState,
) -> Result<Ordering> {
    todo!()
    // let (mut left_values, mut right_values) = match (left.queue.front(), right.queue.front()) {
    //     (Some(left), Some(right)) => (left.iter_order_by_values(), right.iter_order_by_values()),
    //     (None, Some(_)) => return Ok(Ordering::Less),
    //     (Some(_), None) => return Ok(Ordering::Greater),
    //     (None, None) => return Ok(Ordering::Equal),
    // };
    // for ordering in orderings {
    //     let (Some(left), Some(right)) = (left_values.next(), right_values.next()) else {
    //         return Err(ErrorKind::QueryPlanInvalid
    //             .with_message("items have inconsistent numbers of order by items"));
    //     };
    //     let order = left.cmp(&right);
    //     let order = match ordering {
    //         SortOrder::Ascending => order,
    //         SortOrder::Descending => order.reverse(),
    //     };
    //     if order != Ordering::Equal {
    //         // The values are different, so we can return the order
    //         return Ok(order);
    //     }
    // }

    // // The values have been equal so far, so compare the partition keys
    // Ok(left.pkrange.min_inclusive.cmp(&right.pkrange.min_inclusive))
}

impl MergeStrategy {
    pub fn next_partition<'a>(
        &self,
        partitions: &'a mut [PartitionState],
    ) -> Result<Option<&'a mut PartitionState>> {
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

    fn compare_partitions(
        &self,
        left: &PartitionState,
        right: &PartitionState,
    ) -> Result<Ordering> {
        match self {
            MergeStrategy::Unordered => {
                Ok(left.pkrange.min_inclusive.cmp(&right.pkrange.min_inclusive))
            }
            MergeStrategy::Ordered(ordering) => compare_partitions_by_items(&ordering, left, right),
        }
    }
}
