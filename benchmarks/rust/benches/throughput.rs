//! Benchmarks the raw throughput of the query pipeline under various conditions.
//!
//! This benchmark measures the pipeline's throughput in items per second for:
//! - **Unordered queries**: Simple queries without ORDER BY clauses
//! - **Ordered queries**: Queries with ORDER BY clauses that require sorting results across partitions
//!   
//! The ordered variant uses a simple ascending sort on an integer field and validates
//! that results are returned in the correct order.

use azure_data_cosmos_engine::query::{
    JsonQueryClauseItem, PartitionKeyRange, QueryInfo, QueryPipeline, QueryPlan, QueryResult,
    SortOrder,
};
use criterion::{BenchmarkId, Criterion, Throughput};
use std::collections::HashMap;

// Configuration constants
const PARTITION_COUNT: usize = 4;
const PAGE_SIZE: usize = 100;

type RawQueryPipeline = QueryPipeline<Box<serde_json::value::RawValue>, JsonQueryClauseItem>;

// Simple test item for benchmarking
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct BenchmarkItem {
    id: String,
    partition_key: String,
    value: i32,
    description: String,
}

impl BenchmarkItem {
    fn new(id: &str, partition_key: &str, value: i32) -> Self {
        Self {
            id: id.to_string(),
            partition_key: partition_key.to_string(),
            value,
            description: format!("Item {} in partition {}", id, partition_key),
        }
    }
}

impl From<BenchmarkItem> for QueryResult<BenchmarkItem, JsonQueryClauseItem> {
    fn from(item: BenchmarkItem) -> Self {
        QueryResult::from_payload(item)
    }
}

// Benchmark scenario configuration
struct BenchmarkScenario {
    name: &'static str,
    query_sql: &'static str,
    query_plan_fn: fn() -> QueryPlan,
    data_generator_fn: fn(&str, usize) -> Vec<BenchmarkItem>,
}

impl BenchmarkScenario {
    fn new(
        name: &'static str,
        query_sql: &'static str,
        query_plan_fn: fn() -> QueryPlan,
        data_generator_fn: fn(&str, usize) -> Vec<BenchmarkItem>,
    ) -> Self {
        Self {
            name,
            query_sql,
            query_plan_fn,
            data_generator_fn,
        }
    }
}

// Helper function to create test data for a partition
fn create_partition_data(partition_id: &str, item_count: usize) -> Vec<BenchmarkItem> {
    (0..item_count)
        .map(|i| BenchmarkItem::new(&format!("item_{}", i), partition_id, i as i32).into())
        .collect()
}

// Helper to create a simple unordered query plan
fn create_simple_query_plan() -> QueryPlan {
    QueryPlan {
        partitioned_query_execution_info_version: 1,
        query_info: Default::default(), // Uses default QueryInfo which has no ORDER BY, etc.
        query_ranges: Vec::new(),
    }
}

// Helper to create a simple ordered query plan
fn create_ordered_query_plan() -> QueryPlan {
    QueryPlan {
        partitioned_query_execution_info_version: 1,
        query_info: QueryInfo {
            order_by: vec![SortOrder::Ascending],
            ..Default::default()
        },
        query_ranges: Vec::new(),
    }
}

// Helper to create partition key ranges
fn create_partition_key_ranges(count: usize) -> Vec<PartitionKeyRange> {
    (0..count)
        .map(|i| {
            PartitionKeyRange::new(
                format!("partition_{}", i),
                format!("{:02X}", i * 10),
                format!("{:02X}", (i + 1) * 10),
            )
        })
        .collect()
}

// Helper to fulfill data requests from the pipeline
fn fulfill_data_requests(
    requests: &[azure_data_cosmos_engine::query::DataRequest],
    partition_data: &HashMap<String, Vec<BenchmarkItem>>,
    pipeline: &mut RawQueryPipeline,
) {
    for request in requests {
        let pkrange_id = request.pkrange_id.as_ref();
        if let Some(partition_data) = partition_data.get(pkrange_id) {
            // Calculate which items to return based on continuation
            let start_index = request
                .continuation
                .as_ref()
                .and_then(|c| c.parse::<usize>().ok())
                .unwrap_or(0);

            let end_index = std::cmp::min(start_index + PAGE_SIZE, partition_data.len());
            let items: Vec<_> = partition_data[start_index..end_index].to_vec();

            // Because we want to be able to compare this benchmark against the wrappers in Go and Python, we have to generate JSON strings
            // for each item, as the Go and Python benchmarks do.
            // Then, we parse them with pipeline.deserialize_payload, which is what the wrapper code does.
            let items = items.into_iter()
                .map(|i| format!("{{\"id\":\"{}\",\"partition_key\":\"{}\",\"value\":{},\"description\":\"{}\"}}",
                    i.id, i.partition_key, i.value, i.description))
                .collect::<Vec<_>>();

            // Format this into a single response
            let json = format!("{{\"Documents\":[{}]}}", items.join(","));

            // Determine continuation token
            let continuation = if end_index < partition_data.len() {
                Some(end_index.to_string())
            } else {
                None
            };

            // Now deserialize the items into QueryResult
            let result = pipeline
                .deserialize_payload(&json)
                .expect("Failed to deserialize payload");

            // Provide data to pipeline
            pipeline
                .provide_data(pkrange_id, result, continuation)
                .expect("Failed to provide data");
        }
    }
}

// Helper to run a benchmark scenario
fn run_benchmark_scenario(
    scenario: &BenchmarkScenario,
    items_per_partition: usize,
    iters: u64,
) -> std::time::Duration {
    // Pre-create test data once per benchmark configuration
    let partition_data_template: HashMap<String, Vec<BenchmarkItem>> = (0..PARTITION_COUNT)
        .map(|i| {
            let partition_id = format!("partition_{}", i);
            let data = (scenario.data_generator_fn)(&partition_id, items_per_partition);
            (partition_id, data)
        })
        .collect();

    let start = std::time::Instant::now();

    for _ in 0..iters {
        // Create query plan and partition ranges
        let query_plan = (scenario.query_plan_fn)();
        let partition_ranges = create_partition_key_ranges(PARTITION_COUNT);

        // Create pipeline, and use the "raw" query pipeline to emulate the behavior of the wrappers.
        let mut pipeline: RawQueryPipeline =
            QueryPipeline::new(scenario.query_sql, query_plan, partition_ranges)
                .expect("Failed to create pipeline");

        let mut total_items = 0;

        // Run the pipeline until completion
        loop {
            let result = pipeline.run().expect("Pipeline run failed");

            // Count items yielded by this turn
            total_items += result.items.len();

            // If pipeline is terminated, we're done
            if result.terminated {
                break;
            }

            // Fulfill data requests
            fulfill_data_requests(&result.requests, &partition_data_template, &mut pipeline);
        }

        // Verify we processed all expected items
        assert_eq!(total_items, PARTITION_COUNT * items_per_partition);
    }

    start.elapsed()
}

// Main benchmark function
pub fn throughput(c: &mut Criterion) {
    // Test with different numbers of items per partition
    let items_per_partition_values = [100];

    // Define benchmark scenarios
    let scenarios = [
        BenchmarkScenario::new(
            "unordered",
            "SELECT * FROM c",
            create_simple_query_plan,
            create_partition_data,
        ),
        // BenchmarkScenario::new(
        //     "ordered",
        //     "SELECT * FROM c ORDER BY c.value",
        //     create_ordered_query_plan,
        //     create_partition_data,
        // ),
    ];

    for &items_per_partition in &items_per_partition_values {
        let total_items = PARTITION_COUNT * items_per_partition;

        let mut group = c.benchmark_group("pipeline_throughput");
        group.throughput(Throughput::Elements(total_items as u64));

        for scenario in &scenarios {
            group.bench_with_input(
                BenchmarkId::new(scenario.name, items_per_partition),
                &items_per_partition,
                |b, &items_per_partition| {
                    b.iter_custom(|iters| {
                        run_benchmark_scenario(scenario, items_per_partition, iters)
                    });
                },
            );
        }

        group.finish();
    }
}

criterion::criterion_group!(benches, throughput);
criterion::criterion_main!(benches);
