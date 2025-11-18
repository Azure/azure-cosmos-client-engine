// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use std::collections::VecDeque;

use crate::{
    ErrorKind, query::{DataRequest, QueryChunk, QueryResult, node::PipelineNodeResult, query_result::QueryResultShape}
};

use super::{create_query_chunk_states, state::QueryChunkState};

#[derive(Debug)]
pub struct ReadManyStrategy {
    pub current_query_chunk_index: usize,
    pub query_chunk_states: Vec<QueryChunkState>,
    pub items: VecDeque<QueryResult>,
    pub result_shape: QueryResultShape,
}

impl ReadManyStrategy {
    pub fn new(query_chunks: Vec<QueryChunk>) -> Self {
        let query_chunk_states = create_query_chunk_states(&query_chunks);
        tracing::debug!("initialized query chunk states: {:?}", query_chunk_states);
        Self {
            current_query_chunk_index: 0,
            query_chunk_states: query_chunk_states,
            items: VecDeque::new(),
            result_shape: QueryResultShape::RawPayload,
        }
    }

    pub fn requests(&mut self) -> Vec<DataRequest> {
        self.query_chunk_states
            .iter()
            .filter_map(|query_chunk_states| query_chunk_states.request())
            .collect()
    }

    pub fn provide_data(
        &mut self,
        request_id: u64,
        data: &[u8],
        continuation: Option<String>,
    ) -> crate::Result<()> {
        // Parse the raw bytes using the result shape
        let parsed_data = self.result_shape.results_from_slice(data)?;
        tracing::debug!(parsed_data = ?parsed_data, "parsed items from data");

        // Add the data to the items queue. There's no ordering to worry about, so we just append the items.
        self.items.extend(parsed_data);
        tracing::debug!("current items queue length: {}", self.items.len());
        tracing::debug!("continuation: {:?}", continuation);

        // Find the query chunk state by request_id (which matches the chunk's index)
        let query_chunk_state = self
            .query_chunk_states
            .iter_mut()
            .find(|state| state.index == request_id as usize)
            .ok_or_else(|| {
                ErrorKind::InternalError.with_message(format!(
                    "no query chunk state found for request_id/index {}",
                    request_id
                ))
            })?;
        query_chunk_state.update_state(continuation);

        Ok(())
    }

    pub fn produce_item(&mut self) -> crate::Result<PipelineNodeResult> {
        let value = self.items.pop_front();
        let terminated = self.items.is_empty()
            && self.query_chunk_states.iter().all(|state| state.done());
            // && self.query_chunk_states[self.current_query_chunk_index].done());
        Ok(PipelineNodeResult { value, terminated })
    }
}
