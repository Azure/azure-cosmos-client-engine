// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use std::collections::{HashMap, VecDeque};

use crate::{
    query::{
        node::PipelineNodeResult, query_result::QueryResultShape, DataRequest, QueryChunk,
        QueryChunkItem, QueryResult,
    },
    ErrorKind,
 };

use super::{create_query_chunk_states, state::QueryChunkState};

#[derive(Debug)]
pub struct ReadManyStrategy {
    pub query_chunk_states: Vec<QueryChunkState>,
    pub query_chunk_items: Vec<QueryChunkItem>,
    pub items: VecDeque<QueryResult>,
}

impl ReadManyStrategy {
    pub fn new(query_chunks: Vec<QueryChunk>, pk_paths: Vec<String>) -> Self {
        let query_chunk_states = create_query_chunk_states(&query_chunks, pk_paths);
        tracing::debug!("initialized query chunk states: {:?}", query_chunk_states);
        // We collect the query chunk items in order to be used for sorting later, since they contain the original item indexes.
        let query_chunk_items = query_chunks
            .into_iter()
            .flat_map(|chunk| chunk.items)
            .collect();
        Self {
            query_chunk_states: query_chunk_states,
            query_chunk_items: query_chunk_items,
            items: VecDeque::new(),
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
        // Add the data to the items queue.
        self.items.extend(parsed_data);

        // Find the query chunk state by request_id (which matches the chunk's index)
        let query_chunk_state = self
            .query_chunk_states.get_mut(request_id as usize)
            .ok_or_else(|| {
                ErrorKind::InternalError.with_message(format!(
                    "no query chunk state found for request_id/index {}",
                    request_id
                ))
            })?;
        // Update the state and capture the done status before dropping the mutable borrow
        query_chunk_state.update_state(continuation);
        // Drop the mutable borrow here explicitly
        let _ = query_chunk_state;

        // if we're all done, we can sort the final list of items
        if self.query_chunk_states.iter().all(|state| state.done()) {
            // id to index lookup to use for sorting
            let id_to_index: HashMap<String, usize> = self
                .query_chunk_items
                .iter()
                .map(|item| (item.id.clone(), item.index))
                .collect();

            let mut items_with_indices: Vec<(usize, QueryResult)> = self
                .items
                .drain(..)
                .filter_map(|query_result| {
                    let id = extract_id_from_query_result(&query_result)?;
                    let original_index = id_to_index.get(&id)?;
                    Some((*original_index, query_result))
                })
                .collect();

            // sort by the original index
            items_with_indices.sort_by_key(|(index, _)| *index);

            // get the final sorted items
            self.items = items_with_indices
                .into_iter()
                .map(|(_, query_result)| query_result)
                .collect();
        }

        Ok(())
    }

    pub fn produce_item(&mut self) -> crate::Result<PipelineNodeResult> {
        let value = self.items.pop_front();
        let terminated =
            self.items.is_empty() && self.query_chunk_states.iter().all(|state| state.done());
        Ok(PipelineNodeResult { value, terminated })
    }
}

fn extract_id_from_query_result(query_result: &QueryResult) -> Option<String> {
    let QueryResult::RawPayload(payload) = query_result else {
        return None;
    };
    let json: serde_json::Value = serde_json::from_str(payload.get()).ok()?;
    json.get("id")?.as_str().map(|s| s.to_string())
}
