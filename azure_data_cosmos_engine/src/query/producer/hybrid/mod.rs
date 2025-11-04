// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use std::collections::{BTreeSet, VecDeque};

use serde::Deserialize;

mod component_state;
mod fusion;
mod models;

use crate::{
    query::{
        node::PipelineNodeResult, plan::HybridSearchQueryInfo, DataRequest, PartitionKeyRange,
        QueryResult, SortOrder,
    },
    ErrorKind,
};

use component_state::ComponentQueryState;
use fusion::QueryResultCollector;
use models::{ComponentQueryResult, GlobalStatistics, HybridRequestId};

enum HybridSearchPhase {
    IssuingGlobalStatisticsQuery,
    AwaitingGlobalStatistics {
        aggregated_global_statistics: Option<GlobalStatistics>,
        remaining_partitions: usize,
    },
    ComponentQueries {
        remaining_component_queries: usize,
        results: QueryResultCollector,
    },
    ResultProduction(VecDeque<QueryResult>),
}

impl HybridSearchPhase {
    pub fn for_component_queries(count: usize) -> Self {
        if count == 1 {
            HybridSearchPhase::ComponentQueries {
                remaining_component_queries: 1,
                results: QueryResultCollector::singleton(),
            }
        } else {
            HybridSearchPhase::ComponentQueries {
                remaining_component_queries: count,
                results: QueryResultCollector::multiple(),
            }
        }
    }
}

impl std::fmt::Debug for HybridSearchPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HybridSearchPhase::IssuingGlobalStatisticsQuery => {
                f.debug_struct("IssuingGlobalStatisticsQuery").finish()
            }
            HybridSearchPhase::AwaitingGlobalStatistics {
                aggregated_global_statistics,
                remaining_partitions,
            } => f
                .debug_struct("AwaitingGlobalStatistics")
                .field("aggregated_global_statistics", aggregated_global_statistics)
                .field("remaining_partitions", remaining_partitions)
                .finish(),
            HybridSearchPhase::ComponentQueries {
                remaining_component_queries,
                results,
            } => f
                .debug_struct("ComponentQueries")
                .field("remaining_component_queries", remaining_component_queries)
                .field("results_count", &results.len())
                .finish(),
            HybridSearchPhase::ResultProduction(results) => f
                .debug_struct("ResultProduction")
                .field("results_count", &results.len())
                .finish(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct PaginationParameters {
    skip: u64,
    take: u64,
}

impl PaginationParameters {
    pub fn paginate(
        self,
        results: impl IntoIterator<Item = ComponentQueryResult>,
    ) -> VecDeque<QueryResult> {
        results
            .into_iter()
            .skip(self.skip as usize)
            .take(self.take as usize)
            .map(|r| QueryResult::RawPayload(r.payload.user_payload))
            .collect()
    }
}

#[derive(Debug)]
pub struct HybridSearchStrategy {
    global_statistics_query: String,
    phase: HybridSearchPhase,
    pkrange_ids: Vec<String>,
    component_queries: Vec<ComponentQueryState>,
    pagination: PaginationParameters,
}

impl HybridSearchStrategy {
    pub fn new(
        pkranges: impl IntoIterator<Item = PartitionKeyRange>,
        query_info: HybridSearchQueryInfo,
    ) -> crate::Result<Self> {
        let phase = if query_info.requires_global_statistics {
            HybridSearchPhase::IssuingGlobalStatisticsQuery
        } else {
            HybridSearchPhase::for_component_queries(query_info.component_query_infos.len())
        };
        let pkrange_ids: Vec<String> = pkranges.into_iter().map(|p| p.id).collect();

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
        Ok(Self {
            global_statistics_query: query_info.global_statistics_query,
            phase,
            pkrange_ids,
            component_queries,
            pagination: PaginationParameters {
                skip: query_info.skip.unwrap_or(0),
                take: query_info.take.ok_or_else(|| {
                    ErrorKind::InvalidQuery
                        .with_message("hybrid search query must include take parameter")
                })?,
            },
        })
    }

    pub fn requests(&mut self) -> crate::Result<Vec<DataRequest>> {
        match self.phase {
            HybridSearchPhase::IssuingGlobalStatisticsQuery => {
                let requests = self
                    .pkrange_ids
                    .iter()
                    .enumerate()
                    .map(|(_, pkrange_id)| {
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
            HybridSearchPhase::ComponentQueries { .. } => {
                let mut requests = Vec::new();
                for query_state in &self.component_queries {
                    let query_requests = query_state.requests();
                    requests.extend(query_requests);
                }
                Ok(requests)
            }
            // No more requests should be made once we are producing results, but it's not an error to check for them.
            HybridSearchPhase::ResultProduction(_) => Ok(Vec::new()),
        }
    }

    pub fn provide_data(
        &mut self,
        pkrange_id: &str,
        request_id: u64,
        data: &[u8],
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

                #[derive(Deserialize)]
                struct GlobalStatisticsResult {
                    #[serde(rename = "Documents")]
                    documents: Vec<GlobalStatistics>,
                }
                let results =
                    serde_json::from_slice::<GlobalStatisticsResult>(data).map_err(|e| {
                        ErrorKind::DeserializationError.with_message(format!(
                            "failed to deserialize global statistics result: {}",
                            e
                        ))
                    })?;

                if results.documents.len() != 1 {
                    return Err(ErrorKind::InvalidGatewayResponse
                        .with_message("global statistics query should have only one item"));
                }
                let stats = results
                    .documents
                    .into_iter()
                    .next()
                    .expect("we just checked the length");
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
                    self.phase =
                        HybridSearchPhase::for_component_queries(self.component_queries.len());

                    for query_state in &mut self.component_queries {
                        global_statistics.rewrite_component_query(&mut query_state.query_info)?;
                    }
                } else {
                    self.phase = HybridSearchPhase::AwaitingGlobalStatistics {
                        aggregated_global_statistics: Some(global_statistics),
                        remaining_partitions: *remaining_partitions,
                    };
                }
                Ok(())
            }
            HybridSearchPhase::ComponentQueries {
                ref mut remaining_component_queries,
                ref mut results,
            } => {
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
                component_query.update_partition_state(pkrange_id, continuation)?;
                results.provide_data(data);
                if component_query.complete() {
                    *remaining_component_queries -= 1;
                }
                if *remaining_component_queries == 0 {
                    tracing::debug!("all component queries complete");

                    // Swap out the results collector to take ownership of the results.
                    // A brand new singleton collector contains only an empty vector (no heap allocation) so it's cheap to create.
                    let results = std::mem::replace(results, QueryResultCollector::singleton());

                    // Process the results and move to result production
                    let results =
                        results.compute_final_results(self.pagination, &self.component_queries)?;
                    self.phase = HybridSearchPhase::ResultProduction(results);
                }
                Ok(())
            }
            HybridSearchPhase::ResultProduction(_) => {
                crate::debug_panic!("provide_data should not be called in ResultProduction phase");
            }
        }
    }

    pub fn produce_item(&mut self) -> crate::Result<PipelineNodeResult> {
        if let HybridSearchPhase::ResultProduction(ref mut results) = self.phase {
            if let Some(item) = results.pop_front() {
                tracing::debug!("producing hybrid search result item");
                Ok(PipelineNodeResult::result(item.clone(), results.is_empty()))
            } else {
                tracing::debug!("no more hybrid search result items to produce");
                Ok(PipelineNodeResult {
                    value: None,
                    terminated: true,
                })
            }
        } else {
            tracing::debug!(
                "cannot produce items until in ResultProduction phase, current phase: {:?}",
                self.phase
            );
            Ok(PipelineNodeResult::NO_RESULT)
        }
    }
}
