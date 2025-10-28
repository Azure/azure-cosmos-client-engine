use std::{borrow::Cow, vec};

use azure_data_cosmos::query;
use serde::Deserialize;

use crate::{
    query::{
        node::PipelineNodeResult,
        plan::HybridSearchQueryInfo,
        producer::state::{PaginationState, PartitionState},
        DataRequest, PartitionKeyRange, QueryInfo, QueryResult,
    },
    ErrorKind,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GlobalStatistics {
    document_count: u64,
    full_text_statistics: Vec<FullTextStatistics>,
}
impl GlobalStatistics {
    fn aggregate_with(mut self, stats: GlobalStatistics) -> crate::Result<GlobalStatistics> {
        self.document_count += stats.document_count;
        if self.full_text_statistics.len() != stats.full_text_statistics.len() {
            return Err(ErrorKind::InvalidGatewayResponse
                .with_message("mismatched full text statistics length during aggregation"));
        }
        for (a, b) in self
            .full_text_statistics
            .iter_mut()
            .zip(stats.full_text_statistics.iter())
        {
            a.total_word_count += b.total_word_count;
            if a.hit_counts.len() != b.hit_counts.len() {
                return Err(ErrorKind::InvalidGatewayResponse
                    .with_message("mismatched hit counts length during aggregation"));
            }
            for (hit_a, hit_b) in a.hit_counts.iter_mut().zip(b.hit_counts.iter()) {
                *hit_a += *hit_b;
            }
        }
        Ok(self)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FullTextStatistics {
    total_word_count: u64,
    hit_counts: Vec<u64>,
}

#[derive(Debug)]
enum HybridSearchPhase {
    IssuingGlobalStatisticsQuery,
    AwaitingGlobalStatistics {
        aggregated_global_statistics: Option<GlobalStatistics>,
        remaining_partitions: usize,
    },
    ComponentQueries,
}

struct ComponentQueryState {
    query_index: u32,
    query_info: QueryInfo,
    weight: f64,
    partition_states: Vec<(String, PaginationState)>,
    results: Vec<QueryResult>,
}

impl std::fmt::Debug for ComponentQueryState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ComponentQueryState")
            .field("query_index", &self.query_index)
            .field("partition_states", &self.partition_states)
            .field("results_count", &self.results.len())
            .finish()
    }
}

impl ComponentQueryState {
    pub fn new(
        query_index: u32,
        query_info: QueryInfo,
        weight: f64,
        pkrange_ids: &[String],
    ) -> Self {
        Self {
            query_index,
            query_info,
            weight,
            partition_states: pkrange_ids
                .iter()
                .map(|pkrange_id| (pkrange_id.clone(), PaginationState::Initial))
                .collect(),
            results: Vec::new(),
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

    fn provide_data(
        &mut self,
        pkrange_id: &str,
        data: Vec<QueryResult>,
        continuation: Option<String>,
    ) -> Result<(), crate::Error> {
        let partition_state = self
            .partition_states
            .iter_mut()
            .find(|(id, _)| id == pkrange_id)
            .ok_or_else(|| {
                ErrorKind::UnknownPartitionKeyRange
                    .with_message(format!("unknown partition key range ID: {pkrange_id}"))
            })?;
        partition_state.1.update(continuation);
        self.results.extend(data);
        Ok(())
    }

    fn complete(&self) -> bool {
        self.partition_states
            .iter()
            .all(|(_, state)| matches!(state, PaginationState::Done))
    }
}

#[derive(Debug)]
pub struct HybridSearchStrategy {
    global_statistics_query: String,
    phase: HybridSearchPhase,
    pkrange_ids: Vec<String>,
    component_queries: Vec<ComponentQueryState>,
}

/// A unique identifier for a hybrid search query request.
///
/// In order to correlate incoming responses to the appropriate query, we encode both the partition key range index
/// and the component query index into a single u64 value. We start the component query index at 1 to distinguish between
/// global statistics queries (which have an index of 0) and component queries.
///
/// We use the high 32 bits for the partition key range index and the low 32 bits for the component query index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HybridRequestId(u64);

impl From<u64> for HybridRequestId {
    fn from(value: u64) -> Self {
        HybridRequestId(value)
    }
}

impl Into<u64> for HybridRequestId {
    fn into(self) -> u64 {
        self.0
    }
}

impl HybridRequestId {
    pub const GLOBAL_STATISTICS_QUERY_ID: HybridRequestId = HybridRequestId(0);

    /// Creates a request ID for a component query.
    pub fn for_component_query(query_index: u32, page_number: u32) -> Self {
        let id = ((query_index as u64) << 32) | (page_number as u64 + 1);
        HybridRequestId(id)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }

    /// Gets the query index from the request ID, if applicable.
    pub fn query_index(&self) -> Option<u32> {
        if self.0 == 0 {
            None
        } else {
            Some((self.0 >> 32) as u32)
        }
    }

    /// Gets the zero-based page number from the request ID.
    pub fn page_number(&self) -> u32 {
        (self.0 & 0xFFFFFFFF) as u32 - 1
    }
}

impl HybridSearchStrategy {
    pub fn new(
        pkranges: impl IntoIterator<Item = PartitionKeyRange>,
        query_info: HybridSearchQueryInfo,
    ) -> Self {
        let phase = if query_info.requires_global_statistics {
            HybridSearchPhase::IssuingGlobalStatisticsQuery
        } else {
            HybridSearchPhase::ComponentQueries
        };
        let pkrange_ids: Vec<String> = pkranges.into_iter().map(|p| p.id).collect();

        assert_eq!(
            1,
            query_info.component_query_infos.len(),
            "TODO: currently only one component query is supported"
        );

        let component_queries = query_info
            .component_query_infos
            .into_iter()
            .enumerate()
            .map(|(i, q)| {
                ComponentQueryState::new(
                    i as u32,
                    q,
                    query_info.component_weights.get(i).copied().unwrap_or(1.0),
                    &pkrange_ids,
                )
            })
            .collect();
        Self {
            global_statistics_query: query_info.global_statistics_query,
            phase,
            pkrange_ids,
            component_queries,
        }
    }

    pub fn requests(&mut self) -> crate::Result<Vec<DataRequest>> {
        match self.phase {
            HybridSearchPhase::IssuingGlobalStatisticsQuery => {
                let requests = self
                    .pkrange_ids
                    .iter()
                    .enumerate()
                    .map(|(i, pkrange_id)| {
                        DataRequest::with_query(
                            HybridRequestId::GLOBAL_STATISTICS_QUERY_ID.into(),
                            pkrange_id.clone(),
                            None,
                            self.global_statistics_query.clone(),
                            true,
                        )
                    })
                    .collect::<Vec<_>>();
                self.phase = HybridSearchPhase::AwaitingGlobalStatistics {
                    aggregated_global_statistics: None,
                    remaining_partitions: self.pkrange_ids.len(),
                };
                Ok(requests)
            }
            HybridSearchPhase::AwaitingGlobalStatistics { .. } => {
                crate::debug_panic!("no requests should be made in AwaitingGlobalStatistics phase");
            }
            HybridSearchPhase::ComponentQueries => {
                let mut requests = Vec::new();
                for query_state in &self.component_queries {
                    let query_requests = query_state.requests();
                    requests.extend(query_requests);
                }
                Ok(requests)
            }
        }
    }

    pub fn provide_data(
        &mut self,
        pkrange_id: &str,
        request_id: u64,
        mut data: Vec<QueryResult>,
        continuation: Option<String>,
    ) -> Result<(), crate::Error> {
        let request_id = HybridRequestId::from(request_id);
        match self.phase {
            HybridSearchPhase::IssuingGlobalStatisticsQuery => {
                crate::debug_panic!(
                    "provide_data should not be called in IssuingGlobalStatisticsQuery phase"
                );
            }
            HybridSearchPhase::AwaitingGlobalStatistics {
                ref mut aggregated_global_statistics,
                ref mut remaining_partitions,
            } => {
                if request_id != HybridRequestId::GLOBAL_STATISTICS_QUERY_ID {
                    return Err(ErrorKind::InvalidGatewayResponse
                        .with_message("expected global statistics query response"));
                }
                if data.len() != 1 {
                    return Err(ErrorKind::InvalidGatewayResponse
                        .with_message("global statistics query should have only one item"));
                }
                let payload = data.pop().unwrap().payload.ok_or_else(|| {
                    ErrorKind::InvalidGatewayResponse
                        .with_message("global statistics query result should contain a payload")
                })?;
                let stats: GlobalStatistics = serde_json::from_str(payload.get()).map_err(|e| {
                    ErrorKind::DeserializationError
                        .with_message(format!("failed to deserialize global statistics: {}", e))
                })?;
                tracing::trace!(
                    ?stats,
                    pkrange_id,
                    "received global statistics for hybrid search"
                );
                let global_statistics = match aggregated_global_statistics.take() {
                    None => stats,
                    Some(existing_stats) => existing_stats.aggregate_with(stats)?,
                };
                *remaining_partitions -= 1;
                if *remaining_partitions == 0 {
                    // We've received all the global statistics results.
                    // Rewrite component queries with aggregated global statistics
                    tracing::debug!(
                        "received all global statistics results, rewriting component queries"
                    );
                    self.phase = HybridSearchPhase::ComponentQueries;

                    for query_state in &mut self.component_queries {
                        rewrite_component_query(&mut query_state.query_info, &global_statistics)?;
                    }
                } else {
                    self.phase = HybridSearchPhase::AwaitingGlobalStatistics {
                        aggregated_global_statistics: Some(global_statistics),
                        remaining_partitions: *remaining_partitions,
                    };
                }
                Ok(())
            }
            HybridSearchPhase::ComponentQueries => {
                let query_index = request_id.query_index().ok_or_else(|| {
                    ErrorKind::InvalidRequestId.with_message("expected component query request ID")
                })?;
                tracing::trace!(
                    query_index,
                    pkrange_id,
                    "providing data for component query"
                );
                let component_query = self
                    .component_queries
                    .get_mut(query_index as usize)
                    .ok_or_else(|| {
                        ErrorKind::InvalidRequestId
                            .with_message("invalid component query index in request ID")
                    })?;
                component_query.provide_data(pkrange_id, data, continuation)
            }
        }
    }

    pub fn produce_item(&mut self) -> crate::Result<PipelineNodeResult> {
        if self.component_queries.iter().any(|q| !q.complete()) {
            tracing::debug!("cannot produce item: not all component queries are complete");
            return Ok(PipelineNodeResult::NO_RESULT);
        }

        todo!("implement hybrid search result merging and ranking");
    }
}

fn rewrite_component_query(
    query_info: &mut QueryInfo,
    global_statistics: &GlobalStatistics,
) -> crate::Result<()> {
    for order_by_expression in &mut query_info.order_by_expressions {
        *order_by_expression = format_query(order_by_expression, global_statistics)?;
    }
    query_info.rewritten_query = format_query(&query_info.rewritten_query, global_statistics)?;
    Ok(())
}

const TOTAL_DOCUMENT_COUNT: &str = "{documentdb-formattablehybridsearchquery-totaldocumentcount}";
const FORMATTABLE_ORDER_BY: &str = "{documentdb-formattableorderbyquery-filter}";

fn format_query(query: &str, global_statistics: &GlobalStatistics) -> crate::Result<String> {
    // Shortcut for empty query
    if query.is_empty() {
        return Ok(String::new());
    }

    let mut rewritten_query = None;
    for (i, stats) in global_statistics.full_text_statistics.iter().enumerate() {
        let total_word_count_placeholder = format!(
            "{{documentdb-formattablehybridsearchquery-totalwordcount-{}}}",
            i
        );
        let hit_counts_array_placeholder = format!(
            "{{documentdb-formattablehybridsearchquery-hitcountsarray-{}}}",
            i
        );

        let hit_counts = stats
            .hit_counts
            .iter()
            .map(|count| count.to_string())
            .collect::<Vec<_>>()
            .join(",");

        let input_query = rewritten_query.as_deref().unwrap_or(query);
        let new_query = input_query
            .replace(
                &total_word_count_placeholder,
                &stats.total_word_count.to_string(),
            )
            .replace(&hit_counts_array_placeholder, &format!("[{}]", hit_counts));
        rewritten_query = Some(new_query);
    }

    let input_query = rewritten_query.as_deref().unwrap_or(query);
    let final_query = input_query
        .replace(
            TOTAL_DOCUMENT_COUNT,
            &global_statistics.document_count.to_string(),
        )
        .replace(FORMATTABLE_ORDER_BY, "true");
    tracing::trace!(final_query = ?final_query, "rewrote hybrid search query to incorporate global statistics");
    Ok(final_query)
}
