// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use std::collections::BinaryHeap;

use crate::{
    query::{node::PipelineNodeResult, DataRequest, PartitionKeyRange, QueryResult, SortOrder},
    ErrorKind,
};

use super::{
    create_partition_state,
    sorting::{SortableResult, Sorting},
    state::PartitionState,
};

pub struct NonStreamingStrategy {
    pub partitions: Vec<PartitionState>,
    pub sorting: Sorting,
    pub items: BinaryHeap<SortableResult>,
}

impl NonStreamingStrategy {
    pub fn new(
        pkranges: impl IntoIterator<Item = PartitionKeyRange>,
        sorting: Vec<SortOrder>,
    ) -> Self {
        let partitions = create_partition_state(pkranges);
        Self {
            partitions,
            sorting: Sorting::new(sorting),
            items: BinaryHeap::new(),
        }
    }

    pub fn requests(&mut self) -> Option<Vec<DataRequest>> {
        let requests = self
            .partitions
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

    pub fn provide_data(
        &mut self,
        pkrange_id: &str,
        data: Vec<QueryResult>,
        continuation: Option<String>,
    ) -> crate::Result<()> {
        // Insert the items into the heap as we go, which will keep them sorted
        for item in data {
            // We need to sort the items by the order by items, so we create a SortableResult.
            self.items
                .push(SortableResult::new(self.sorting.clone(), item));
        }

        // Update the partition state with the continuation token
        let partition = self
            .partitions
            .iter_mut()
            .find(|p| p.pkrange.id == pkrange_id)
            .ok_or_else(|| {
                ErrorKind::UnknownPartitionKeyRange
                    .with_message(format!("unknown partition key range ID: {pkrange_id}"))
            })?;
        partition.update_state(continuation);

        Ok(())
    }

    pub fn produce_item(&mut self) -> crate::Result<PipelineNodeResult> {
        // We can only produce items when all partitions are done.
        if self.partitions.iter().any(|p| !p.done()) {
            // If any partition is not done, we cannot produce items yet.
            tracing::debug!("not all partitions are done, cannot produce items");
            return Ok(PipelineNodeResult::NO_RESULT);
        }

        // We can just pop the next item from the heap, since it's already sorted.
        let value = self.items.pop().map(|r| r.into());
        Ok(PipelineNodeResult {
            value,
            terminated: self.items.is_empty(),
        })
    }
}
