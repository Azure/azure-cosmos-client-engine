//! Benchmarks the deserialization performance of QueryResult under various conditions.
//!
//! This benchmark measures the impact of different payload sizes, order by clause counts,
//! and whether the query result is "rewritten" (deserialized directly) or "not rewritten"
//! (payload deserialized separately and wrapped in QueryResult::from_payload).

use azure_data_cosmos_engine::query::{JsonQueryClauseItem, QueryResult};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use serde_json::value::RawValue;
use std::hint::black_box;

type BenchmarkedQueryResult = QueryResult<Box<RawValue>, JsonQueryClauseItem>;

/// Generates a JSON payload object with a "data" property containing random data of approximately the specified size.
fn generate_payload_json(target_size: usize) -> String {
    // Generate random data string to fill the target size
    let data_content = "x".repeat(target_size);
    serde_json::json!({
        "id": "test-item",
        "partitionKey": "test-partition",
        "data": data_content
    }).to_string()
}

/// Generates order by items as JSON objects with the form {"item": value}.
fn generate_order_by_items(count: usize) -> String {
    if count == 0 {
        return "[]".to_string();
    }
    
    let items: Vec<String> = (0..count)
        .map(|i| format!(r#"{{"item": {}}}"#, i))
        .collect();
    
    format!("[{}]", items.join(","))
}

/// Generates a complete QueryResult JSON for "rewritten" scenarios.
fn generate_rewritten_json(payload_size: usize, order_by_count: usize) -> String {
    let payload = generate_payload_json(payload_size);
    let order_by_items = generate_order_by_items(order_by_count);
    
    format!(
        r#"{{"orderByItems": {}, "payload": {}}}"#,
        order_by_items, payload
    )
}

/// Benchmark parameters
#[derive(Clone)]
struct BenchmarkParams {
    payload_size: usize,
    payload_size_name: &'static str,
    order_by_count: usize,
    is_rewritten: bool,
}

impl BenchmarkParams {
    fn benchmark_name(&self) -> String {
        if self.is_rewritten {
            format!(
                "rewritten/{}/order_by_{}",
                self.payload_size_name, self.order_by_count
            )
        } else {
            format!("not_rewritten/{}", self.payload_size_name)
        }
    }
}

fn benchmark_rewritten_deserialization(c: &mut Criterion, params: &BenchmarkParams) {
    let json_data = generate_rewritten_json(params.payload_size, params.order_by_count);
    let json_size = json_data.len();
    
    let mut group = c.benchmark_group("query_result_deserialization");
    group.throughput(Throughput::Bytes(json_size as u64));
    
    group.bench_with_input(
        BenchmarkId::from_parameter(params.benchmark_name()),
        &json_data,
        |b, json| {
            b.iter(|| {
                let result: BenchmarkedQueryResult = black_box(
                    serde_json::from_str(json).expect("Failed to deserialize QueryResult")
                );
                black_box(result)
            })
        },
    );
    
    group.finish();
}

fn benchmark_not_rewritten_deserialization(c: &mut Criterion, params: &BenchmarkParams) {
    let payload_json = generate_payload_json(params.payload_size);
    let json_size = payload_json.len();
    
    let mut group = c.benchmark_group("query_result_deserialization");
    group.throughput(Throughput::Bytes(json_size as u64));
    
    group.bench_with_input(
        BenchmarkId::from_parameter(params.benchmark_name()),
        &payload_json,
        |b, json| {
            b.iter(|| {
                let payload: Box<RawValue> = black_box(
                    serde_json::from_str(json).expect("Failed to deserialize payload")
                );
                let result: BenchmarkedQueryResult = black_box(
                    QueryResult::from_payload(payload)
                );
                black_box(result)
            })
        },
    );
    
    group.finish();
}

fn query_result_deserialization_benchmarks(c: &mut Criterion) {
    let payload_sizes = vec![
        (10, "10b"),
        (1024, "1kb"),
    ];
    
    let order_by_counts = vec![0, 1, 2];
    
    // Generate all benchmark parameter combinations
    let mut all_params = Vec::new();
    
    // Rewritten scenarios (with all order by count variations)
    for (payload_size, payload_size_name) in &payload_sizes {
        for &order_by_count in &order_by_counts {
            all_params.push(BenchmarkParams {
                payload_size: *payload_size,
                payload_size_name,
                order_by_count,
                is_rewritten: true,
            });
        }
    }
    
    // Not rewritten scenarios (always 0 order by items)
    for (payload_size, payload_size_name) in &payload_sizes {
        all_params.push(BenchmarkParams {
            payload_size: *payload_size,
            payload_size_name,
            order_by_count: 0,
            is_rewritten: false,
        });
    }
    
    // Run all benchmarks
    for params in &all_params {
        if params.is_rewritten {
            benchmark_rewritten_deserialization(c, params);
        } else {
            benchmark_not_rewritten_deserialization(c, params);
        }
    }
}

criterion_group!(benches, query_result_deserialization_benchmarks);
criterion_main!(benches);
