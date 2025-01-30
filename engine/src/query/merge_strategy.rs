use std::cmp::Ordering;

use super::{PartitionState, SortOrder};
use crate::Result;

pub enum MergeStrategy {
    Ordered(Vec<SortOrder>),
    Unordered,
}

impl MergeStrategy {
    pub fn next_item<'a>(&self, partitions: &'a mut [PartitionState]) -> ! {
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

        todo!("Return batch of next items, or request for more data, or completion")
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
            MergeStrategy::Ordered(orderings) => {
                let (left_item, right_item) = match (left.queue.front(), right.queue.front()) {
                    (Some(left), Some(right)) => (left, right),
                    (None, Some(_)) => return Ok(Ordering::Less),
                    (Some(_), None) => return Ok(Ordering::Greater),
                    (None, None) => return Ok(Ordering::Equal),
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
