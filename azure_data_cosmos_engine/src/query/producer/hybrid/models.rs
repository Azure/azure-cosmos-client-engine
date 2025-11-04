// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

// cSpell:ignore formattablehybridsearchquery formattableorderbyquery hitcountsarray totalwordcount totaldocumentcount

use serde::Deserialize;

use crate::{query::QueryInfo, ErrorKind};

/// A unique identifier for a hybrid search query request.
///
/// In order to correlate incoming responses to the appropriate query, we encode both the partition key range index
/// and the component query index into a single u64 value. We start the component query index at 1 to distinguish between
/// global statistics queries (which have an index of 0) and component queries.
///
/// We use the high 32 bits for the partition key range index and the low 32 bits for the component query index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HybridRequestId(u64);

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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalStatistics {
    pub document_count: u64,
    pub full_text_statistics: Vec<FullTextStatistics>,
}

const TOTAL_DOCUMENT_COUNT: &str = "{documentdb-formattablehybridsearchquery-totaldocumentcount}";
const FORMATTABLE_ORDER_BY: &str = "{documentdb-formattableorderbyquery-filter}";

impl GlobalStatistics {
    pub fn aggregate_with(mut self, stats: GlobalStatistics) -> crate::Result<GlobalStatistics> {
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

    pub fn rewrite_component_query(&self, query: &mut QueryInfo) -> crate::Result<()> {
        for order_by_expression in &mut query.order_by_expressions {
            *order_by_expression = self.apply_to_query_template(order_by_expression)?;
        }
        query.rewritten_query = self.apply_to_query_template(&query.rewritten_query)?;
        Ok(())
    }

    fn apply_to_query_template(&self, query: &str) -> crate::Result<String> {
        // Shortcut for empty query
        if query.is_empty() {
            return Ok(String::new());
        }

        let mut rewritten_query = None;
        for (i, stats) in self.full_text_statistics.iter().enumerate() {
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
            .replace(TOTAL_DOCUMENT_COUNT, &self.document_count.to_string())
            .replace(FORMATTABLE_ORDER_BY, "true");
        tracing::trace!(final_query = ?final_query, "rewrote hybrid search query to incorporate global statistics");
        Ok(final_query)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FullTextStatistics {
    pub total_word_count: u64,
    pub hit_counts: Vec<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentQueryResult {
    #[serde(rename = "_rid")]
    pub rid: String,
    pub payload: ComponentQueryPayload,
}

// Implement ordering, and equality based on the rid field only.
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
pub struct ComponentQueryPayload {
    pub component_scores: Vec<f64>,
    #[serde(rename = "payload")]
    pub user_payload: Box<serde_json::value::RawValue>,
}
