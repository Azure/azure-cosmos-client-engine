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
    // EPK range: 0x40000000 to 0x7FFFFFFF (since EPKs are divided evenly among 4 partitions)
    let query_plan = QueryPlan {
        partitioned_query_execution_info_version: 1,
        query_info: QueryInfo::default(),
        query_ranges: vec![QueryRange {
            min: "40000000".to_string(), // Start of partition1's range
            max: "7FFFFFFF".to_string(), // End of partition1's range
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
