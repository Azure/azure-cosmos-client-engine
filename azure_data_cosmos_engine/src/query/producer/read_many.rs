// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use std::collections::VecDeque;

use crate::{
    ErrorKind, query::{DataRequest, QueryChunk, QueryChunkItem, QueryResult, node::PipelineNodeResult, query_result::QueryResultShape}
};

use super::{create_query_chunk_states, state::QueryChunkState};

#[derive(Debug)]
pub struct ReadManyStrategy {
    pub query_chunk_states: Vec<QueryChunkState>,
    pub query_chunk_items: Vec<QueryChunkItem>,
    pub items: VecDeque<QueryResult>,
}

impl ReadManyStrategy {
    pub fn new(query_chunks: Vec<QueryChunk>) -> Self {
        let query_chunk_states = create_query_chunk_states(&query_chunks);
        tracing::debug!("initialized query chunk states: {:?}", query_chunk_states);
        // We collect the query chunk items in order to be used for sorting later, since they contain the original item indexes.
        let query_chunk_items = query_chunks.into_iter().flat_map(|chunk| chunk.items).collect();
        Self {
            query_chunk_states: query_chunk_states,
            query_chunk_items: query_chunk_items,
            items: VecDeque::new()
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
        let parsed_data = QueryResultShape::RawPayload.results_from_slice(data)?;
        tracing::debug!(parsed_data = ?parsed_data, "parsed items from data");

        // Add the data to the items queue.
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
        // Verify all query chunks are done, and if so sort the items to be returned
        query_chunk_state.update_state(continuation);
        tracing::debug!("random item: {:?}", self.items[0]);
        // if self.query_chunk_states.iter().all(|state| state.done()) {
        //     // id to index lookup to use for sorting
        //     let id_to_index: HashMap<String, usize> = self.query_chunk_items
        //         .iter()
        //         .map(|item| (item.id.clone(), item.index))
        //         .collect();

        //     let mut items_with_indices: Vec<(usize, QueryResult)> = self.items
        //         .drain(..)
        //         .filter_map(|query_result| {
        //             let id = extract_id_from_query_result(&query_result)?;
        //             let original_index = id_to_index.get(&id)?;
        //             Some((*original_index, query_result))
        //         })
        //         .collect();

        //     // sort by the original index
        //     items_with_indices.sort_by_key(|(index, _)| *index);

        //     // get the final sorted items
        //     self.items = items_with_indices.into_iter()
        //         .map(|(_, query_result)| query_result)
        //         .collect();
        // }
        tracing::debug!("query chunk state: {}", query_chunk_state.done());
        tracing::debug!("state of all chunks: {}", self.query_chunk_states.iter().all(|state| state.done()));

        Ok(())
    }

    pub fn produce_item(&mut self) -> crate::Result<PipelineNodeResult> {
        let value = self.items.pop_front();
        let terminated = self.items.is_empty()
            && self.query_chunk_states.iter().all(|state| state.done());
        Ok(PipelineNodeResult { value, terminated })
    }
}
