use std::cmp::Ordering;

use serde::de;

use crate::query::{DataRequest, PartitionKeyRange};

/// Represents the current stage of pagination for a partition.
#[derive(Debug)]
pub enum PaginationState {
    /// The partition is ready for the first data request. There should be no data in the queue yet.
    Initial,

    /// The partition has a pending continuation. When the current queue is exhausted, the continuation can be used to fetch more data.
    Continuing(String),

    /// The partition has been exhausted. When the current queue is exhausted, the partition is done.
    Done,
}

#[derive(Debug)]
pub struct PartitionState {
    /// The index of the partition in the pkranges list used by the pipeline.
    pub index: usize,
    /// The partition key range this state is for.
    pub pkrange: PartitionKeyRange,
    /// The current stage of pagination for this partition.
    pub stage: PaginationState,
}

impl PartialEq for PartitionState {
    fn eq(&self, other: &Self) -> bool {
        self.pkrange.id == other.pkrange.id
    }
}

impl Eq for PartitionState {}

impl PartialOrd for PartitionState {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.pkrange
            .min_inclusive
            .partial_cmp(&other.pkrange.min_inclusive)
    }
}

impl Ord for PartitionState {
    fn cmp(&self, other: &Self) -> Ordering {
        self.pkrange.min_inclusive.cmp(&other.pkrange.min_inclusive)
    }
}

impl PartitionState {
    /// Initializes a partition state for the given partition key range.
    pub fn new(index: usize, pkrange: PartitionKeyRange) -> Self {
        Self {
            index,
            pkrange,
            stage: PaginationState::Initial,
        }
    }

    /// Gets the next [`DataRequest`] for this partition, if one is needed.
    pub fn request(&self) -> Option<DataRequest> {
        match &self.stage {
            PaginationState::Initial => Some(DataRequest {
                pkrange_id: self.pkrange.id.clone().into(),
                continuation: None,
            }),
            PaginationState::Continuing(token) => Some(DataRequest {
                pkrange_id: self.pkrange.id.clone().into(),
                continuation: Some(token.clone()),
            }),
            PaginationState::Done => None,
        }
    }

    pub fn update_state(&mut self, continuation: Option<String>) {
        match continuation {
            Some(token) => {
                self.stage = PaginationState::Continuing(token);
            }
            None => {
                self.stage = PaginationState::Done;
            }
        }
    }

    pub fn started(&self) -> bool {
        !matches!(self.stage, PaginationState::Initial)
    }

    pub fn done(&self) -> bool {
        matches!(self.stage, PaginationState::Done)
    }
}
