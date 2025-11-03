use std::{
    collections::{BTreeSet, BinaryHeap, VecDeque},
    hash::Hash,
};

use serde::Deserialize;

use crate::{
    query::{
        node::PipelineNodeResult, plan::HybridSearchQueryInfo, producer::state::PaginationState,
        DataRequest, PartitionKeyRange, QueryClauseItem, QueryInfo, QueryResult, SortOrder,
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ComponentQueryResult {
    #[serde(rename = "_rid")]
    rid: String,
    payload: ComponentQueryPayload,
}

// Implement hashing, ordering, and equality based on the rid field only.
impl Hash for ComponentQueryResult {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.rid.hash(state);
    }
}

impl PartialOrd for ComponentQueryResult {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.rid.cmp(&other.rid))
    }
}

impl Ord for ComponentQueryResult {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.rid.cmp(&other.rid)
    }
}

impl PartialEq for ComponentQueryResult {
    fn eq(&self, other: &Self) -> bool {
        self.rid == other.rid
    }
}

impl Eq for ComponentQueryResult {}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ComponentQueryPayload {
    component_scores: Vec<f64>,
    #[serde(rename = "payload")]
    user_payload: Box<serde_json::value::RawValue>,
}

enum QueryResultCollector {
    /// Collects results from a single component query.
    /// There's no need to de-duplicate results in this case.
    Singleton(Vec<ComponentQueryResult>),

    /// Collects results from multiple component queries.
    /// Results must be de-duplicated based on their RID.
    Multiple(BTreeSet<ComponentQueryResult>),
}

impl QueryResultCollector {
    fn singleton() -> Self {
        QueryResultCollector::Singleton(Vec::new())
    }

    fn multiple() -> Self {
        QueryResultCollector::Multiple(BTreeSet::new())
    }

    fn len(&self) -> usize {
        match self {
            QueryResultCollector::Singleton(v) => v.len(),
            QueryResultCollector::Multiple(s) => s.len(),
        }
    }

    fn extend<I: IntoIterator<Item = ComponentQueryResult>>(&mut self, items: I) {
        match self {
            QueryResultCollector::Singleton(v) => v.extend(items),
            QueryResultCollector::Multiple(s) => s.extend(items),
        }
    }
}

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

#[derive(Debug)]
struct ComponentQueryState {
    query_index: u32,
    query_info: QueryInfo,
    weight: f64,
    partition_states: Vec<(String, PaginationState)>,
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

    fn complete(&self) -> bool {
        self.partition_states
            .iter()
            .all(|(_, state)| matches!(state, PaginationState::Done))
    }

    fn update_partition_state(
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
        state.1.update(continuation);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct PaginationParameters {
    skip: u64,
    take: u64,
}

pub struct HybridSearchStrategy {
    global_statistics_query: String,
    phase: HybridSearchPhase,
    pkrange_ids: Vec<String>,
    component_queries: Vec<ComponentQueryState>,
    pagination: PaginationParameters,
}

impl std::fmt::Debug for HybridSearchStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HybridSearchStrategy")
            .field("phase", &self.phase)
            .field("pkrange_ids", &self.pkrange_ids)
            .field("component_queries", &self.component_queries)
            .field("pagination", &self.pagination)
            .finish()
    }
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

    /// Gets the query index from the request ID, if applicable.
    pub fn query_index(&self) -> Option<u32> {
        if self.0 == 0 {
            None
        } else {
            Some((self.0 >> 32) as u32)
        }
    }
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

                #[derive(Deserialize)]
                struct ComponentQueryResults {
                    #[serde(rename = "Documents")]
                    documents: Vec<ComponentQueryResult>,
                }
                let result =
                    serde_json::from_slice::<ComponentQueryResults>(data).map_err(|e| {
                        ErrorKind::DeserializationError.with_message(format!(
                            "failed to deserialize component query result: {}",
                            e
                        ))
                    })?;

                let component_query = self
                    .component_queries
                    .get_mut(query_index as usize)
                    .ok_or_else(|| {
                        ErrorKind::InvalidRequestId
                            .with_message("invalid component query index in request ID")
                    })?;
                component_query.update_partition_state(pkrange_id, continuation)?;
                results.extend(result.documents);
                if component_query.complete() {
                    *remaining_component_queries -= 1;
                }
                if *remaining_component_queries == 0 {
                    tracing::debug!("all component queries complete");

                    // Process the results and move to result production
                    let results =
                        compute_final_results(self.pagination, results, &self.component_queries)?;
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

fn compute_final_results(
    pagination: PaginationParameters,
    results: &mut QueryResultCollector,
    component_queries: &[ComponentQueryState],
) -> crate::Result<VecDeque<QueryResult>> {
    match results {
        QueryResultCollector::Singleton(results) => {
            let r = std::mem::take(results);
            format_final_results(pagination, r.into_iter())
        }
        QueryResultCollector::Multiple(results) => {
            let results = std::mem::take(results);
            let scores = get_scores(component_queries, &results);
            let ranks = rank_scores(&scores);

            // There's currently no way to iterate a BinaryHeap in sorted order without popping elements.
            // There's a plan for this in the future, but it's not yet stabilized
            // (and it's not very clear it would be any better anyway since the current implementation of that does essentially what we do here).
            // See https://github.com/rust-lang/rust/issues/59278
            let fused = fuse_ranks(&ranks, component_queries, results);
            
            // Use stable sort to preserve BTreeSet order for tied scores
            let mut fused_vec: Vec<_> = fused.into_iter().collect();
            fused_vec.sort_by(|a, b| {
                // Sort by fused score descending (highest first)
                b.fused_score
                    .partial_cmp(&a.fused_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            let final_results = fused_vec.into_iter().map(|r| r.result);
            format_final_results(pagination, final_results)
        }
    }
}

#[derive(Debug)]
struct RankFusionResult {
    fused_score: f64,
    result: ComponentQueryResult,
}

impl PartialEq for RankFusionResult {
    fn eq(&self, other: &Self) -> bool {
        self.fused_score == other.fused_score
    }
}

impl Eq for RankFusionResult {}

impl PartialOrd for RankFusionResult {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RankFusionResult {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Only compare by fused score - stable sort will handle ties
        other
            .fused_score
            .partial_cmp(&self.fused_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    }
}

const RRF_CONSTANT: f64 = 60.0;

fn fuse_ranks(
    ranks: &[Vec<usize>],
    component_queries: &[ComponentQueryState],
    results: BTreeSet<ComponentQueryResult>,
) -> BinaryHeap<RankFusionResult> {
    let mut fused_results = BinaryHeap::new();

    for (index, result) in results.into_iter().enumerate() {
        let mut fused_score = 0.0;
        for (component_index, rank_list) in ranks.iter().enumerate() {
            let rank = rank_list[index] as f64;
            let weight = component_queries[component_index].weight;
            fused_score += weight / (RRF_CONSTANT + rank);
        }
        fused_results.push(RankFusionResult {
            fused_score,
            result,
        });
    }

    fused_results
}

fn rank_scores(score_lists: &[Vec<(f64, usize)>]) -> Vec<Vec<usize>> {
    let mut ranks = vec![vec![0; score_lists[0].len()]; score_lists.len()];

    // The scores are in order, so all we have to do is assign ranks based on position.
    // But, two identical scores should receive the same rank.
    for (component_index, score_list) in score_lists.iter().enumerate() {
        let mut current_rank = 1;
        for i in 0..score_list.len() {
            if i > 0 && (score_list[i].0 - score_list[i - 1].0).abs() > f64::EPSILON {
                current_rank = i + 1;
            }
            ranks[component_index][score_list[i].1] = current_rank;
        }
    }

    ranks
}

fn get_scores(
    component_queries: &[ComponentQueryState],
    results: &BTreeSet<ComponentQueryResult>,
) -> Vec<Vec<(f64, usize)>> {
    // In debug builds, verify that all results have the expected number of component scores
    #[cfg(debug_assertions)]
    {
        for result in results.iter() {
            debug_assert_eq!(
                result.payload.component_scores.len(),
                component_queries.len(),
                "mismatched number of component scores in hybrid search result"
            );
        }
    }

    let mut score_list = vec![Vec::new(); component_queries.len()];
    for (index, result) in results.iter().enumerate() {
        for (component_index, score) in result.payload.component_scores.iter().copied().enumerate()
        {
            score_list[component_index].push((score, index));
        }
    }

    // Sort each component's scores according to its sort order
    for (index, tuples) in score_list.iter_mut().enumerate() {
        // let sort_order = component_queries[index]
        //     .query_info
        //     .order_by
        //     .first()
        //     .copied()
        //     .expect("a single ordering");
        // let sort_fn = if sort_order == SortOrder::Descending {
        //     |a: &(f64, usize), b: &(f64, usize)| {
        //         b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal)
        //     }
        // } else {
        //     |a: &(f64, usize), b: &(f64, usize)| {
        //         a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal)
        //     }
        // };
        let sort_fn = |a: &(f64, usize), b: &(f64, usize)| {
            b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal)
        };
        tuples.sort_by(sort_fn);
    }
    score_list
}

fn format_final_results(
    pagination: PaginationParameters,
    results: impl Iterator<Item = ComponentQueryResult>,
) -> crate::Result<VecDeque<QueryResult>> {
    // We implement skip/take natively here to trim back the results stored in memory before returning them.
    Ok(results
        .skip(pagination.skip as usize)
        .take(pagination.take as usize)
        .map(|r| QueryResult::RawPayload(r.payload.user_payload))
        .collect())
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
