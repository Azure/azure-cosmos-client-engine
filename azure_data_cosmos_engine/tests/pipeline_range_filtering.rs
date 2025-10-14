// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use std::vec;

use azure_data_cosmos_engine::query::{
    DataRequest, JsonQueryClauseItem, QueryInfo, QueryPlan, QueryRange, QueryResult,
};
use pretty_assertions::assert_eq;

use mock_engine::{Container, Engine};

mod mock_engine;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Item {
    id: String,
    partition_key: String,
    title: String,
}

impl Item {
    pub fn new(id: impl Into<String>, partition_key: impl Into<String>) -> Self {
        let id = id.into();
        let partition_key = partition_key.into();
        let title = format!("{}/{}", partition_key.clone(), id.clone());
        Self {
            id,
            partition_key,
            title,
        }
    }
}

impl From<Item> for QueryResult<Item, JsonQueryClauseItem> {
    fn from(item: Item) -> Self {
        QueryResult::from_payload(item)
    }
}

#[test]
pub fn pkranges_filtered_by_query_ranges() -> Result<(), Box<dyn std::error::Error>> {
    let mut container = Container::new();

    // Set up 4 physical partitions with data
    container.insert(
        "partition0",
        vec![
            Item::new("item0", "partition0").into(),
            Item::new("item1", "partition0").into(),
            Item::new("item2", "partition0").into(),
        ],
    );
    container.insert(
        "partition1",
        vec![
            Item::new("item0", "partition1").into(),
            Item::new("item1", "partition1").into(),
            Item::new("item2", "partition1").into(),
        ],
    );
    container.insert(
        "partition2",
        vec![
            Item::new("item0", "partition2").into(),
            Item::new("item1", "partition2").into(),
            Item::new("item2", "partition2").into(),
        ],
    );
    container.insert(
        "partition3",
        vec![
            Item::new("item0", "partition3").into(),
            Item::new("item1", "partition3").into(),
            Item::new("item2", "partition3").into(),
        ],
    );

    // Create a query plan with query ranges that only cover partition1
    // Based on the Engine::new implementation in mock_engine/mod.rs, partition1 will have
    // EPK range: 0x3FFFFFFF to 0x7FFFFFFF (since EPKs are divided evenly among 4 partitions)
    let query_plan = QueryPlan {
        partitioned_query_execution_info_version: 1,
        query_info: QueryInfo::default(),
        query_ranges: vec![QueryRange {
            min: "40000000".to_string(), // Start within partition1's range
            max: "7FFFFFFC".to_string(), // End within partition1's range (well before partition2)
            is_min_inclusive: true,
            is_max_inclusive: true,
        }],
    };

    let engine = Engine::new(
        container,
        "SELECT * FROM c WHERE c.partitionKey = 'specific_value'",
        query_plan,
        3,
    )?;

    // Execute the query
    let results = engine.execute()?;

    // Extract just the requests to validate partition filtering
    let all_requests: Vec<&DataRequest> = results
        .iter()
        .flat_map(|response| &response.requests)
        .collect();

    // Verify that only partition1 was requested
    // This test SHOULD FAIL because the current implementation doesn't filter by query ranges
    let requested_partitions: std::collections::HashSet<&str> = all_requests
        .iter()
        .map(|req| req.pkrange_id.as_ref())
        .collect();

    assert_eq!(
        requested_partitions,
        std::collections::HashSet::from(["partition1"]),
        "Expected only partition1 to be queried based on query ranges, but got: {:?}",
        requested_partitions
    );

    // Also verify that we get results only from partition1
    let all_items: Vec<String> = results
        .into_iter()
        .flat_map(|response| response.items)
        .map(|item| item.title)
        .collect();

    let expected_items = vec![
        "partition1/item0".to_string(),
        "partition1/item1".to_string(),
        "partition1/item2".to_string(),
    ];

    assert_eq!(
        all_items, expected_items,
        "Expected only items from partition1, but got: {:?}",
        all_items
    );

    Ok(())
}

#[test]
pub fn pkranges_filtered_by_overlapping_query_ranges() -> Result<(), Box<dyn std::error::Error>> {
    let mut container = Container::new();

    // Set up 4 physical partitions with data
    container.insert(
        "partition0",
        vec![
            Item::new("item0", "partition0").into(),
            Item::new("item1", "partition0").into(),
            Item::new("item2", "partition0").into(),
        ],
    );
    container.insert(
        "partition1",
        vec![
            Item::new("item0", "partition1").into(),
            Item::new("item1", "partition1").into(),
            Item::new("item2", "partition1").into(),
        ],
    );
    container.insert(
        "partition2",
        vec![
            Item::new("item0", "partition2").into(),
            Item::new("item1", "partition2").into(),
            Item::new("item2", "partition2").into(),
        ],
    );
    container.insert(
        "partition3",
        vec![
            Item::new("item0", "partition3").into(),
            Item::new("item1", "partition3").into(),
            Item::new("item2", "partition3").into(),
        ],
    );

    // Create a query plan with query ranges that overlap partition1 and partition2
    // Based on the Engine::new implementation in mock_engine/mod.rs:
    // partition1 EPK range: 0x40000000 to 0x7FFFFFFF
    // partition2 EPK range: 0x80000000 to 0xBFFFFFFF
    // We'll create a range that starts partway through partition1 and ends partway through partition2
    let query_plan = QueryPlan {
        partitioned_query_execution_info_version: 1,
        query_info: QueryInfo::default(),
        query_ranges: vec![QueryRange {
            min: "60000000".to_string(), // Partway through partition1's range
            max: "A0000000".to_string(), // Partway through partition2's range
            is_min_inclusive: true,
            is_max_inclusive: true,
        }],
    };

    let engine = Engine::new(
        container,
        "SELECT * FROM c WHERE c.someField BETWEEN 'value1' AND 'value2'",
        query_plan,
        3,
    )?;

    // Execute the query
    let results = engine.execute()?;

    // Extract just the requests to validate partition filtering
    let all_requests: Vec<&DataRequest> = results
        .iter()
        .flat_map(|response| &response.requests)
        .collect();

    // Verify that only partition1 and partition2 were requested
    // This test SHOULD FAIL because the current implementation doesn't filter by query ranges
    let requested_partitions: std::collections::HashSet<&str> = all_requests
        .iter()
        .map(|req| req.pkrange_id.as_ref())
        .collect();

    assert_eq!(
        requested_partitions,
        std::collections::HashSet::from(["partition1", "partition2"]),
        "Expected only partition1 and partition2 to be queried based on overlapping query ranges, but got: {:?}",
        requested_partitions
    );

    // Also verify that we get results only from partition1 and partition2
    let all_items: Vec<String> = results
        .into_iter()
        .flat_map(|response| response.items)
        .map(|item| item.title)
        .collect();

    let mut expected_items = vec![
        "partition1/item0".to_string(),
        "partition1/item1".to_string(),
        "partition1/item2".to_string(),
        "partition2/item0".to_string(),
        "partition2/item1".to_string(),
        "partition2/item2".to_string(),
    ];
    // Sort both vectors since order might vary
    expected_items.sort();
    let mut sorted_items = all_items;
    sorted_items.sort();

    assert_eq!(
        sorted_items, expected_items,
        "Expected only items from partition1 and partition2, but got: {:?}",
        sorted_items
    );

    Ok(())
}

#[test]
pub fn pkranges_filtered_by_all_partitions_query_range() -> Result<(), Box<dyn std::error::Error>> {
    let mut container = Container::new();

    // Set up 4 physical partitions with data
    container.insert(
        "partition0",
        vec![
            Item::new("item0", "partition0").into(),
            Item::new("item1", "partition0").into(),
            Item::new("item2", "partition0").into(),
        ],
    );
    container.insert(
        "partition1",
        vec![
            Item::new("item0", "partition1").into(),
            Item::new("item1", "partition1").into(),
            Item::new("item2", "partition1").into(),
        ],
    );
    container.insert(
        "partition2",
        vec![
            Item::new("item0", "partition2").into(),
            Item::new("item1", "partition2").into(),
            Item::new("item2", "partition2").into(),
        ],
    );
    container.insert(
        "partition3",
        vec![
            Item::new("item0", "partition3").into(),
            Item::new("item1", "partition3").into(),
            Item::new("item2", "partition3").into(),
        ],
    );

    // Create a query plan with query ranges that cover all partitions
    // Based on the Engine::new implementation in mock_engine/mod.rs:
    // The EPK space goes from 0x00000000 to 0xFFFFFFFF
    let query_plan = QueryPlan {
        partitioned_query_execution_info_version: 1,
        query_info: QueryInfo::default(),
        query_ranges: vec![QueryRange {
            min: "00000000".to_string(), // Start of EPK space
            max: "FFFFFFFF".to_string(), // End of EPK space
            is_min_inclusive: true,
            is_max_inclusive: true,
        }],
    };

    let engine = Engine::new(container, "SELECT * FROM c", query_plan, 3)?;

    // Execute the query
    let results = engine.execute()?;

    // Extract just the requests to validate partition filtering
    let all_requests: Vec<&DataRequest> = results
        .iter()
        .flat_map(|response| &response.requests)
        .collect();

    // Verify that all partitions were requested
    // This test should actually PASS when range filtering is implemented, since the range covers all partitions
    let requested_partitions: std::collections::HashSet<&str> = all_requests
        .iter()
        .map(|req| req.pkrange_id.as_ref())
        .collect();

    assert_eq!(
        requested_partitions,
        std::collections::HashSet::from(["partition0", "partition1", "partition2", "partition3"]),
        "Expected all partitions to be queried when query range covers all partitions, but got: {:?}",
        requested_partitions
    );

    // Also verify that we get results from all partitions
    let all_items: Vec<String> = results
        .into_iter()
        .flat_map(|response| response.items)
        .map(|item| item.title)
        .collect();

    let mut expected_items = vec![
        "partition0/item0".to_string(),
        "partition0/item1".to_string(),
        "partition0/item2".to_string(),
        "partition1/item0".to_string(),
        "partition1/item1".to_string(),
        "partition1/item2".to_string(),
        "partition2/item0".to_string(),
        "partition2/item1".to_string(),
        "partition2/item2".to_string(),
        "partition3/item0".to_string(),
        "partition3/item1".to_string(),
        "partition3/item2".to_string(),
    ];
    // Sort both vectors since order might vary
    expected_items.sort();
    let mut sorted_items = all_items;
    sorted_items.sort();

    assert_eq!(
        sorted_items, expected_items,
        "Expected items from all partitions, but got: {:?}",
        sorted_items
    );

    Ok(())
}

#[test]
pub fn pkranges_no_query_ranges_queries_all_partitions() -> Result<(), Box<dyn std::error::Error>> {
    let mut container = Container::new();

    // Set up 4 physical partitions with data
    container.insert(
        "partition0",
        vec![
            Item::new("item0", "partition0").into(),
            Item::new("item1", "partition0").into(),
            Item::new("item2", "partition0").into(),
        ],
    );
    container.insert(
        "partition1",
        vec![
            Item::new("item0", "partition1").into(),
            Item::new("item1", "partition1").into(),
            Item::new("item2", "partition1").into(),
        ],
    );
    container.insert(
        "partition2",
        vec![
            Item::new("item0", "partition2").into(),
            Item::new("item1", "partition2").into(),
            Item::new("item2", "partition2").into(),
        ],
    );
    container.insert(
        "partition3",
        vec![
            Item::new("item0", "partition3").into(),
            Item::new("item1", "partition3").into(),
            Item::new("item2", "partition3").into(),
        ],
    );

    // Create a query plan with NO query ranges (empty vector)
    // This should query all partitions
    let query_plan = QueryPlan {
        partitioned_query_execution_info_version: 1,
        query_info: QueryInfo::default(),
        query_ranges: Vec::new(), // No query ranges
    };

    let engine = Engine::new(container, "SELECT * FROM c", query_plan, 3)?;

    // Execute the query
    let results = engine.execute()?;

    // Extract just the requests to validate partition filtering
    let all_requests: Vec<&DataRequest> = results
        .iter()
        .flat_map(|response| &response.requests)
        .collect();

    // Verify that all partitions were requested
    // This test should actually PASS when range filtering is implemented, since no ranges means query all
    let requested_partitions: std::collections::HashSet<&str> = all_requests
        .iter()
        .map(|req| req.pkrange_id.as_ref())
        .collect();

    assert_eq!(
        requested_partitions,
        std::collections::HashSet::from(["partition0", "partition1", "partition2", "partition3"]),
        "Expected all partitions to be queried when no query ranges are specified, but got: {:?}",
        requested_partitions
    );

    // Also verify that we get results from all partitions
    let all_items: Vec<String> = results
        .into_iter()
        .flat_map(|response| response.items)
        .map(|item| item.title)
        .collect();

    let mut expected_items = vec![
        "partition0/item0".to_string(),
        "partition0/item1".to_string(),
        "partition0/item2".to_string(),
        "partition1/item0".to_string(),
        "partition1/item1".to_string(),
        "partition1/item2".to_string(),
        "partition2/item0".to_string(),
        "partition2/item1".to_string(),
        "partition2/item2".to_string(),
        "partition3/item0".to_string(),
        "partition3/item1".to_string(),
        "partition3/item2".to_string(),
    ];
    // Sort both vectors since order might vary
    expected_items.sort();
    let mut sorted_items = all_items;
    sorted_items.sort();

    assert_eq!(
        sorted_items, expected_items,
        "Expected items from all partitions, but got: {:?}",
        sorted_items
    );

    Ok(())
}
