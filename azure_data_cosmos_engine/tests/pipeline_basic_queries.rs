// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use std::vec;

use azure_data_cosmos_engine::query::{DataRequest, QueryPlan, QueryResult, QueryResultShape};
use pretty_assertions::assert_eq;

use mock_engine::{Container, Engine};
use serde_json::json;

use crate::mock_engine::EngineResult;

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
        let value = serde_json::value::to_raw_value(&item.title).unwrap();
        QueryResult {
            payload: Some(value),
            ..Default::default()
        }
    }
}

#[test]
pub fn unordered_query() -> Result<(), Box<dyn std::error::Error>> {
    let mut container = Container::new();

    container.insert(
        "partition0",
        vec![
            Item::new("item0", "partition0").into(),
            Item::new("item1", "partition0").into(),
            Item::new("item2", "partition0").into(),
            Item::new("item3", "partition0").into(),
            Item::new("item4", "partition0").into(),
            Item::new("item5", "partition0").into(),
        ],
    );
    container.insert(
        "partition1",
        vec![
            Item::new("item0", "partition1").into(),
            Item::new("item1", "partition1").into(),
            Item::new("item2", "partition1").into(),
            Item::new("item3", "partition1").into(),
            Item::new("item4", "partition1").into(),
            Item::new("item5", "partition1").into(),
        ],
    );

    let engine = Engine::new(
        container,
        "SELECT * FROM c",
        QueryPlan {
            partitioned_query_execution_info_version: 1,
            query_info: Default::default(),
            query_ranges: Vec::new(),
        },
        3,
        QueryResultShape::RawPayload,
    )?;

    // Execute the query, and flatten the response down to just the title for easier comparison.
    let results = engine.execute()?;
    assert_eq!(
        vec![
            EngineResult {
                items: vec![],
                requests: vec![DataRequest::new("partition0", None),],
                terminated: false,
            },
            EngineResult {
                items: vec![
                    json!("partition0/item0"),
                    json!("partition0/item1"),
                    json!("partition0/item2"),
                ],
                requests: vec![DataRequest::new("partition0", Some("3".into())),],
                terminated: false,
            },
            EngineResult {
                items: vec![
                    json!("partition0/item3"),
                    json!("partition0/item4"),
                    json!("partition0/item5"),
                ],
                requests: vec![DataRequest::new("partition1", None),],
                terminated: false,
            },
            EngineResult {
                items: vec![
                    json!("partition1/item0"),
                    json!("partition1/item1"),
                    json!("partition1/item2"),
                ],
                requests: vec![DataRequest::new("partition1", Some("3".into())),],
                terminated: false,
            },
            EngineResult {
                items: vec![
                    json!("partition1/item3"),
                    json!("partition1/item4"),
                    json!("partition1/item5"),
                ],
                requests: vec![],
                terminated: true,
            },
        ],
        results
    );

    Ok(())
}
