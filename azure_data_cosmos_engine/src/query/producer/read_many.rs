// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use std::collections::{HashMap, VecDeque};

use crate::{
    query::{node::PipelineNodeResult, query_result::QueryResultShape, DataRequest, QueryResult},
    ErrorKind,
};

use super::{create_query_chunk_states, state::QueryChunkState};

pub struct ReadManyStrategy {
    pub query_chunks: Vec<HashMap<String, Vec<(usize, String, String)>>>,
    pub current_query_chunk_index: usize,
    pub query_chunk_states: Vec<QueryChunkState>,
    pub items: VecDeque<QueryResult>,
    pub result_shape: QueryResultShape,
}

impl ReadManyStrategy {
    pub fn new(query_chunks: Vec<HashMap<String, Vec<(usize, String, String)>>>) -> Self {
        let query_chunk_states = create_query_chunk_states(&query_chunks);
        Self {
            query_chunks: query_chunks,
            current_query_chunk_index: 0,
            query_chunk_states: query_chunk_states,
            items: VecDeque::new(),
            result_shape: QueryResultShape::RawPayload,
        }
    }

    pub fn requests(&mut self) -> Vec<DataRequest> {
        // Here is where we would create DataRequests for the current query chunk.

        // In the unordered strategy, we simply return the first partition key range's request.
        // Once that partition is exhausted, we remove it from the list and return the next one.
        let mut requests = Vec::new();
        while requests.is_empty() {
            // If there are no more partitions, return None.
            let Some(query_chunk_state) =
                self.query_chunk_states.get(self.current_query_chunk_index)
            else {
                break;
            };
            match query_chunk_state.request() {
                Some(request) => {
                    tracing::trace!(pkrange_id = ?query_chunk_state.pkrange_id, "requesting data for partition");
                    requests.push(request);
                }
                None => {
                    tracing::trace!(pkrange_id = ?query_chunk_state.pkrange_id, "partition exhausted, removing from list");
                    tracing::trace!(current_query_chunk_index = ?self.current_query_chunk_index, "increasing query chunk index");
                    self.current_query_chunk_index += 1;
                }
            }
        }
        requests
    }

    pub fn provide_data(
        &mut self,
        _pkrange_id: &str,
        data: &[u8],
        continuation: Option<String>,
    ) -> crate::Result<()> {
        // Parse the raw bytes using the result shape
        let parsed_data = self.result_shape.results_from_slice(data)?;

        // Add the data to the items queue. There's no ordering to worry about, so we just append the items.
        self.items.extend(parsed_data);

        // Update the query chunk state with the continuation token
        let query_chunk_state = self
            .query_chunk_states
            .get_mut(self.current_query_chunk_index)
            .ok_or_else(|| {
                ErrorKind::InternalError.with_message(format!(
                    "no query chunk state found for index {}",
                    self.current_query_chunk_index
                ))
            })?;
        query_chunk_state.update_state(continuation);

        Ok(())
    }

    pub fn produce_item(&mut self) -> crate::Result<PipelineNodeResult> {
        let value = self.items.pop_front();
        let terminated = self.items.is_empty()
            && (self.current_query_chunk_index == self.query_chunks.len() - 1)
            && self.query_chunk_states[self.current_query_chunk_index].done();
        Ok(PipelineNodeResult { value, terminated })
    }
}
