use std::{borrow::Cow, vec};

use azure_data_cosmos::query;
use serde::Deserialize;

use crate::{
    query::{
        node::PipelineNodeResult, plan::HybridSearchQueryInfo, DataRequest, PartitionKeyRange,
        QueryInfo, QueryResult,
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
struct HybridPartitionState {
    pkrange_id: String,
}

impl HybridPartitionState {
    pub fn new(pkrange_id: String) -> Self {
        Self { pkrange_id }
    }
}

#[derive(Debug)]
enum HybridSearchPhase {
    IssuingGlobalStatisticsQuery,
    AwaitingGlobalStatistics {
        aggregated_global_statistics: Option<GlobalStatistics>,
        remaining_partitions: usize,
    },
    ComponentQueries { results: Vec<QueryResult> },
}

#[derive(Debug)]
pub struct HybridSearchStrategy {
    global_statistics_query: String,
    phase: HybridSearchPhase,
    partitions: Vec<HybridPartitionState>,
    component_queries: Vec<QueryInfo>,
}

impl HybridSearchStrategy {
    pub fn new(
        pkranges: impl IntoIterator<Item = PartitionKeyRange>,
        query_info: HybridSearchQueryInfo,
    ) -> Self {
        let partitions = pkranges
            .into_iter()
            .map(|pkrange| HybridPartitionState::new(pkrange.id))
            .collect();
        let phase = if query_info.requires_global_statistics {
            HybridSearchPhase::IssuingGlobalStatisticsQuery
        } else {
            HybridSearchPhase::ComponentQueries { results: Vec::new() }
        };
        Self {
            global_statistics_query: query_info.global_statistics_query,
            partitions,
            phase,
            component_queries: query_info.component_query_infos,
        }
    }

    pub fn requests(&self) -> Option<Vec<DataRequest>> {
        match self.phase {
            HybridSearchPhase::IssuingGlobalStatisticsQuery => {
                let requests = self
                    .partitions
                    .iter()
                    .map(|partition| {
                        DataRequest::with_query(
                            partition.pkrange_id.clone(),
                            None,
                            self.global_statistics_query.clone(),
                            true,
                        )
                    })
                    .collect::<Vec<_>>();
                self.phase = HybridSearchPhase::AwaitingGlobalStatistics {
                    aggregated_global_statistics: None,
                    remaining_partitions: self.partitions.len(),
                };
                Some(requests)
            }
            HybridSearchPhase::AwaitingGlobalStatistics { .. } => {
                crate::debug_panic!("no requests should be made in AwaitingGlobalStatistics phase")
            },
            HybridSearchPhase::ComponentQueries { .. } => {
            }
        }
    }

    pub fn provide_data(
        &self,
        pkrange_id: &str,
        data: Vec<QueryResult>,
        continuation: Option<String>,
    ) -> Result<(), crate::Error> {
        match self.phase {
            HybridSearchPhase::IssuingGlobalStatisticsQuery => crate::debug_panic!(
                "provide_data should not be called in IssuingGlobalStatisticsQuery phase"
            ),
            HybridSearchPhase::AwaitingGlobalStatistics {
                ref mut aggregated_global_statistics,
                ref mut remaining_partitions,
            } => {
                if data.len() != 1 {
                    return Err(ErrorKind::InvalidGatewayResponse
                        .with_message("global statistics query should have only one item"));
                }
                let payload = data[0].payload.ok_or_else(|| {
                    Err(ErrorKind::InvalidGatewayResponse
                        .with_message("global statistics query result should contain a payload"))
                })?;
                let stats: GlobalStatistics = serde_json::from_str(payload.get()).map_err(|e| {
                    ErrorKind::DeserializationError
                        .with_message(format!("failed to deserialize global statistics: {}", e))
                })?;
                let global_statistics = match aggregated_global_statistics {
                    None => stats,
                    Some(existing_stats) => existing_stats.aggregate_with(stats)?,
                };
                *remaining_partitions -= 1;
                if *remaining_partitions == 0 {
                    // We've received all the global statistics results.
                    // Rewrite component queries with aggregated global statistics
                    self.phase = HybridSearchPhase::ComponentQueries { results: Vec::new() };

                    for query_info in &mut self.component_queries {
                        rewrite_component_query(query_info, &global_statistics)?;
                    }
                } else {
                    *aggregated_global_statistics = Some(global_statistics);
                }
                Ok(())
            }
            HybridSearchPhase::ComponentQueries { ref mut results} => {
            }
        }
    }

    pub fn produce_item(&mut self) -> crate::Result<PipelineNodeResult> {
        todo!()
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

const TOTAL_DOCUMENT_COUNT: &str = "{documentdb-formattablehybridsearchquery-totaldocumentcount}"
const FORMATTABLE_ORDER_BY: &str = "{documentdb-formattableorderbyquery-filter}"

fn format_query(query: &str, global_statistics: &GlobalStatistics) -> crate::Result<String> {
    // Shortcut for empty query
    if query.is_empty() {
        return Ok(String::new());
    }

    let mut rewritten_query = None;
    for (i, stats) in global_statistics
        .full_text_statistics
        .iter()
        .enumerate()
    {
        let total_word_count_placeholder = format!("{{documentdb-formattablehybridsearchquery-totalwordcount-{}}}", i);
        let hit_counts_array_placeholder = format!("{{documentdb-formattablehybridsearchquery-hitcountsarray-{}}}", i);

        let hit_counts = stats
            .hit_counts
            .iter()
            .map(|count| count.to_string())
            .collect::<Vec<_>>()
            .join(",");

        let input_query = rewritten_query.as_deref().unwrap_or(query);
        let new_query = input_query
            .replace(&total_word_count_placeholder, &stats.total_word_count.to_string())
            .replace(&hit_counts_array_placeholder, &format!("[{}]", hit_counts));
        rewritten_query = Some(new_query);
    }

    let input_query = rewritten_query.as_deref().unwrap_or(query);
    let final_query = input_query.replace(
        TOTAL_DOCUMENT_COUNT,
        &global_statistics.document_count.to_string(),
    ).replace(
        FORMATTABLE_ORDER_BY,
        "true",
    );
    tracing::trace!(final_query = ?final_query, "rewrote hybrid search query to incorporate global statistics");
    Ok(final_query)
}
