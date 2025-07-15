// Quick validation script to verify JSON structure
use azure_data_cosmos_engine::query::{JsonQueryClauseItem, QueryResult};
use serde_json::value::RawValue;

type BenchmarkedQueryResult = QueryResult<Box<RawValue>, JsonQueryClauseItem>;

fn main() {
    // Test rewritten JSON generation
    let rewritten_json = r#"{"orderByItems": [{"item": 0}, {"item": 1}], "payload": {"id": "test-item", "partitionKey": "test-partition", "data": "xxxxxxxxxxxx"}}"#;
    
    println!("Testing rewritten JSON:");
    println!("{}", rewritten_json);
    
    let result: BenchmarkedQueryResult = serde_json::from_str(rewritten_json).unwrap();
    println!("Order by items count: {}", result.order_by_items.len());
    println!("Payload: {}", result.payload.get());
    
    // Test not rewritten scenario
    let payload_json = r#"{"id": "test-item", "partitionKey": "test-partition", "data": "xxxxxxxxxxxx"}"#;
    let payload: Box<RawValue> = serde_json::from_str(payload_json).unwrap();
    let result_from_payload: BenchmarkedQueryResult = QueryResult::from_payload(payload);
    
    println!("\nTesting not rewritten scenario:");
    println!("Order by items count: {}", result_from_payload.order_by_items.len());
    println!("Payload: {}", result_from_payload.payload.get());
}
