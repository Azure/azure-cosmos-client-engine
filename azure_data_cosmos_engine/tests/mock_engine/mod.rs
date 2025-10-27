// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

//! This module contains a kind of simulated Cosmos DB backend that can be used to test the query engine.
//!
//! The backend here is VERY simple and depends on a few assumptions:
//! * Partitions are all "physical", there are no logical partitions.
//! * If testing an ORDER BY query, the data in each partition is ALREADY sorted by the ORDER BY field(s).
//! * Partitions are "ordered" by their ID (in Cosmos DB, physical partitions are ordered by the minimum logical partition key value covered by the physical partition).

use std::{collections::BTreeMap, fmt::Debug};

use azure_data_cosmos_engine::query::{
    DataRequest, PartitionKeyRange, QueryPipeline, QueryPlan, QueryResult,
};
use tracing_subscriber::EnvFilter;

pub struct Engine {
    container: Container,
    partitions: Vec<PartitionKeyRange>,
    pipeline: QueryPipeline,
    request_page_size: usize,
}

impl Engine {
    /// Creates a new engine with the given container and query plan.
    ///
    /// # Parameters
    ///
    /// * `container` - The container to query.
    /// * `plan` - The query plan to execute.
    /// * `request_page_size` - Limits the number of items returned in each page of results when querying a partition, see pagination below.
    ///
    /// # Pagination
    ///
    /// NOTE: The `request_page_size` parameter does NOT guarantee that results will be returned in pages of that size.
    /// It only limits the number of items returned in each page of results when querying EACH PARTITION.
    /// So if a two partitions have `request_page_size` items, and can be fully merged and returned without requesting more data, the result will contain `2 * request_page_size` items.
    /// Or, if fewer than `request_page_size` items can be emitted before needing to request more data, the result will contain fewer items.
    ///
    /// It's up to language bindings to handle pagination and buffer data as needed.
    pub fn new(
        container: Container,
        query: &str,
        plan: QueryPlan,
        request_page_size: usize,
    ) -> Result<Self, azure_data_cosmos_engine::Error> {
        // Divide the EPK space evenly among the partitions we have
        const MAX_EPK: u32 = 0xFFFF_FFFF;
        const MIN_EPK: u32 = 0x0000_0000;
        let epks_per_partition = (MAX_EPK - MIN_EPK) / (container.partitions.len() as u32);

        let partitions = container
            .partitions
            .keys()
            .enumerate()
            .map(|(index, pkrange_id)| {
                PartitionKeyRange::new(
                    pkrange_id.clone(),
                    format!("{:08X}", MIN_EPK + (index as u32) * epks_per_partition),
                    if index == container.partitions.len() - 1 {
                        // Last partition gets the rest of the range
                        format!("{:08X}", MAX_EPK)
                    } else {
                        format!(
                            "{:08X}",
                            MIN_EPK + ((index as u32) + 1) * epks_per_partition - 1
                        )
                    },
                )
            });
        let partitions = partitions.collect::<Vec<_>>();
        let pipeline = QueryPipeline::new(query, plan, partitions.clone())?;
        Ok(Engine {
            container,
            partitions,
            pipeline,
            request_page_size,
        })
    }

    pub fn partition_key_ranges(&self) -> &[PartitionKeyRange] {
        &self.partitions
    }

    /// Executes the query, returning the result in individual batches.
    ///
    /// Each separate `Vec<T>` represents a single [`PipelineResponse`] received from the query pipeline.
    /// After each batch, the engine automatically fulfills any requests for additional data from the pipeline and moves to the next batch.
    pub fn execute(mut self) -> Result<Vec<EngineResult>, azure_data_cosmos_engine::Error> {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .try_init();

        let mut responses = Vec::new();
        loop {
            let result = self.pipeline.run()?;

            let result = EngineResult {
                items: result
                    .items
                    .into_iter()
                    .map(|r| serde_json::from_str(r.get()).unwrap())
                    .collect(),
                requests: result.requests,
                terminated: result.terminated,
            };
            responses.push(result.clone());

            if result.terminated {
                break;
            }

            for request in result.requests {
                let page = self.container.get_data(
                    &request.pkrange_id,
                    request.continuation.as_deref(),
                    self.request_page_size,
                );
                self.pipeline
                    .provide_data(&request.pkrange_id, page.items, page.continuation)?;
            }
        }

        Ok(responses)
    }
}

/// Equivalent of [`PipelineResponse`], but with the raw items as [`Value`](serde_json::Value) for easier testing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EngineResult {
    pub items: Vec<serde_json::Value>,
    pub requests: Vec<DataRequest>,
    pub terminated: bool,
}

pub struct Page {
    pub items: Vec<QueryResult>,
    pub continuation: Option<String>,
}

/// Represents a container in the simulated Cosmos DB backend.
///
/// Because we don't need to simulate Database or Account operations, this is the root of the simulated engine.
pub struct Container {
    partitions: BTreeMap<String, Partition>,
}

impl Container {
    pub fn new() -> Self {
        Container {
            partitions: BTreeMap::new(),
        }
    }

    pub fn insert(
        &mut self,
        partition_key: impl Into<String>,
        items: impl IntoIterator<Item = QueryResult>,
    ) {
        let partition_key = partition_key.into();
        self.partitions
            .entry(partition_key)
            .or_insert_with(Partition::new)
            .extend(items);
    }

    pub fn get_data(
        &self,
        partition_key: &str,
        continuation: Option<&str>,
        page_size: usize,
    ) -> Page {
        self.partitions
            .get(partition_key)
            .map(|partition| partition.get_data(continuation, page_size))
            .unwrap_or_else(|| Page {
                items: Vec::new(),
                continuation: None,
            })
    }
}

/// Represents the sequence of pages that will be returned by a given partition.
pub struct Partition {
    data: Vec<QueryResult>,
}

impl Partition {
    pub fn new() -> Self {
        Partition { data: Vec::new() }
    }

    pub fn extend(&mut self, items: impl IntoIterator<Item = QueryResult>) {
        self.data.extend(items)
    }

    pub fn get_data(&self, continuation: Option<&str>, page_size: usize) -> Page {
        let index = continuation
            .map(|c| c.parse::<usize>().unwrap())
            .unwrap_or(0);

        let items = self
            .data
            .iter()
            .skip(index)
            .take(page_size)
            .cloned()
            .collect::<Vec<_>>();

        let end = index + items.len();

        let continuation = if end < self.data.len() {
            Some(end.to_string())
        } else {
            None
        };

        Page {
            items,
            continuation,
        }
    }
}
