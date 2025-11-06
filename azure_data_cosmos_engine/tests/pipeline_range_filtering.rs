// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use std::vec;

use azure_data_cosmos_engine::query::{DataRequest, QueryInfo, QueryPlan, QueryRange, QueryResult};
use pretty_assertions::assert_eq;

use mock_engine::{Container, Engine};
use serde_json::json;

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

impl From<Item> for QueryResult {
    fn from(item: Item) -> Self {
        let raw = serde_json::value::to_raw_value(&item.title).unwrap();
        QueryResult::RawPayload(raw)
    }
}

#[test]
pub fn pkranges_filtered_by_query_ranges() -> Result<(), Box<dyn std::error::Error>> {
    let mut container = Container::new();

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

    // Target partition1 with EPK range that fits within its boundaries
    let query_plan = QueryPlan {
        partitioned_query_execution_info_version: 1,
        query_info: QueryInfo::default(),
        query_ranges: vec![QueryRange {
            min: "40000000".to_string(),
            max: "7FFFFFFC".to_string(), // Avoids boundary overlap with partition2
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

    let results = engine.execute()?;

    let all_requests: Vec<&DataRequest> = results
        .iter()
        .flat_map(|response| &response.requests)
        .collect();

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

    let all_items = results
        .into_iter()
        .flat_map(|response| response.items)
        .collect::<Vec<_>>();

    let expected_items = vec![
        json!("partition1/item0"),
        json!("partition1/item1"),
        json!("partition1/item2"),
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

    // Query range that spans across two adjacent partitions
    let query_plan = QueryPlan {
        partitioned_query_execution_info_version: 1,
        query_info: QueryInfo::default(),
        query_ranges: vec![QueryRange {
            min: "60000000".to_string(),
            max: "A0000000".to_string(),
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

    let results = engine.execute()?;

    let all_requests: Vec<&DataRequest> = results
        .iter()
        .flat_map(|response| &response.requests)
        .collect();

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

    let actual_items = results
        .into_iter()
        .flat_map(|response| response.items)
        .collect::<Vec<_>>();

    let expected_items = vec![
        json!("partition1/item0"),
        json!("partition1/item1"),
        json!("partition1/item2"),
        json!("partition2/item0"),
        json!("partition2/item1"),
        json!("partition2/item2"),
    ];
    // Sort for deterministic comparison since execution order may vary
    assert_eq!(
        actual_items, expected_items,
        "Expected only items from partition1 and partition2, but got: {:?}",
        actual_items
    );

    Ok(())
}

#[test]
pub fn pkranges_filtered_by_all_partitions_query_range() -> Result<(), Box<dyn std::error::Error>> {
    let mut container = Container::new();

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

    // Query range that spans the entire EPK space
    let query_plan = QueryPlan {
        partitioned_query_execution_info_version: 1,
        query_info: QueryInfo::default(),
        query_ranges: vec![QueryRange {
            min: "00000000".to_string(),
            max: "FFFFFFFF".to_string(),
            is_min_inclusive: true,
            is_max_inclusive: true,
        }],
    };

    let engine = Engine::new(container, "SELECT * FROM c", query_plan, 3)?;

    let results = engine.execute()?;

    let all_requests: Vec<&DataRequest> = results
        .iter()
        .flat_map(|response| &response.requests)
        .collect();

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

    let actual_items = results
        .into_iter()
        .flat_map(|response| response.items)
        .collect::<Vec<_>>();

    let expected_items = vec![
        json!("partition0/item0"),
        json!("partition0/item1"),
        json!("partition0/item2"),
        json!("partition1/item0"),
        json!("partition1/item1"),
        json!("partition1/item2"),
        json!("partition2/item0"),
        json!("partition2/item1"),
        json!("partition2/item2"),
        json!("partition3/item0"),
        json!("partition3/item1"),
        json!("partition3/item2"),
    ];

    assert_eq!(
        actual_items, expected_items,
        "Expected items from all partitions, but got: {:?}",
        actual_items
    );

    Ok(())
}

#[test]
pub fn pkranges_no_query_ranges_queries_all_partitions() -> Result<(), Box<dyn std::error::Error>> {
    let mut container = Container::new();

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

    // Empty query ranges should default to querying all partitions
    let query_plan = QueryPlan {
        partitioned_query_execution_info_version: 1,
        query_info: QueryInfo::default(),
        query_ranges: Vec::new(),
    };

    let engine = Engine::new(container, "SELECT * FROM c", query_plan, 3)?;

    let results = engine.execute()?;

    let all_requests: Vec<&DataRequest> = results
        .iter()
        .flat_map(|response| &response.requests)
        .collect();

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

    let actual_items = results
        .into_iter()
        .flat_map(|response| response.items)
        .collect::<Vec<_>>();

    let expected_items = vec![
        json!("partition0/item0"),
        json!("partition0/item1"),
        json!("partition0/item2"),
        json!("partition1/item0"),
        json!("partition1/item1"),
        json!("partition1/item2"),
        json!("partition2/item0"),
        json!("partition2/item1"),
        json!("partition2/item2"),
        json!("partition3/item0"),
        json!("partition3/item1"),
        json!("partition3/item2"),
    ];

    assert_eq!(
        actual_items, expected_items,
        "Expected items from all partitions, but got: {:?}",
        actual_items
    );

    Ok(())
}
