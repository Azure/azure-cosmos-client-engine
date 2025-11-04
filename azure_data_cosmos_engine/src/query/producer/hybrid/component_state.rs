// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use crate::query::{
    producer::{hybrid::models::HybridRequestId, state::PaginationState},
    DataRequest, QueryInfo,
};

#[derive(Debug)]
pub struct ComponentQueryState {
    query_index: u32,
    pub query_info: QueryInfo,
    pub weight: f64,
    partition_states: Vec<(String, PaginationState)>,
    remaining_partitions: usize,
}

impl ComponentQueryState {
    pub fn new(
        query_index: u32,
        query_info: QueryInfo,
        weight: f64,
        pkrange_ids: &[String],
    ) -> Self {
        tracing::trace!(
            query_index,
            weight,
            ?pkrange_ids,
            "creating component query state"
        );
        Self {
            query_index,
            query_info,
            weight,
            partition_states: pkrange_ids
                .iter()
                .map(|pkrange_id| (pkrange_id.clone(), PaginationState::Initial))
                .collect(),
            remaining_partitions: pkrange_ids.len(),
        }
    }

    pub fn requests(&self) -> Vec<DataRequest> {
        let mut requests = Vec::new();
        for (pkrange_id, pagination_state) in &self.partition_states {
            let req = match pagination_state {
                PaginationState::Initial => Some(DataRequest::with_query(
                    HybridRequestId::for_component_query(self.query_index, 0).into(),
                    pkrange_id.clone(),
                    None,
                    self.query_info.rewritten_query.clone(),
                    true,
                )),
                PaginationState::Continuing {
                    next_page_index,
                    token,
                } => Some(DataRequest::with_query(
                    HybridRequestId::for_component_query(self.query_index, *next_page_index as u32)
                        .into(),
                    pkrange_id.clone(),
                    Some(token.clone()),
                    self.query_info.rewritten_query.clone(),
                    true,
                )),
                PaginationState::Done => None,
            };
            if let Some(request) = req {
                requests.push(request);
            }
        }
        requests
    }

    pub fn complete(&self) -> bool {
        let result = self.remaining_partitions == 0;
        debug_assert!(
            !result
                || self
                    .partition_states
                    .iter()
                    .all(|(_, state)| matches!(state, PaginationState::Done))
        );
        result
    }

    pub fn update_partition_state(
        &mut self,
        pkrange_id: &str,
        continuation: Option<String>,
    ) -> crate::Result<()> {
        let state = self
            .partition_states
            .iter_mut()
            .find(|(id, _)| id == pkrange_id)
            .ok_or_else(|| {
                crate::ErrorKind::InvalidGatewayResponse.with_message(format!(
                    "received response for unknown partition key range ID: {}",
                    pkrange_id
                ))
            })?;
        if matches!(state.1, PaginationState::Done) {
            Ok(())
        } else {
            state.1.update(continuation);
            if matches!(state.1, PaginationState::Done) {
                self.remaining_partitions -= 1;
            }
            Ok(())
        }
    }
}
