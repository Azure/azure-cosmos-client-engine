// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use std::vec;

use azure_data_cosmos_engine::query::{
    DataRequest, QueryClauseItem, QueryInfo, QueryPlan, QueryResult, QueryResultShape, SortOrder,
};
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
    sort0: u32,
    sort1: String,
}

impl Item {
    pub fn new(
        id: impl Into<String>,
        partition_key: impl Into<String>,
        sort0: u32,
        sort1: &str,
    ) -> Self {
        let id = id.into();
        let partition_key = partition_key.into();
        let title = format!("{}/{}", partition_key.clone(), id.clone());
        Self {
            id,
            partition_key,
            title,
            sort0,
            sort1: sort1.into(),
        }
    }
}

impl From<Item> for QueryResult {
    fn from(item: Item) -> Self {
        let raw = serde_json::value::to_raw_value(&item.title).unwrap();
        let sort0 = QueryClauseItem::from_value(serde_json::Value::Number(
            serde_json::Number::from(item.sort0),
        ));
        let sort1 = QueryClauseItem::from_value(serde_json::Value::String(item.sort1.clone()));
        QueryResult {
            order_by_items: vec![sort0, sort1],
            payload: Some(raw),
            ..Default::default()
        }
    }
}

#[test]
pub fn top() -> Result<(), Box<dyn std::error::Error>> {
    let mut container = Container::new();

    container.insert(
        "partition0",
        vec![
            Item::new("item0", "partition0", 1, "aaaa").into(),
            Item::new("item1", "partition0", 2, "yyyy").into(),
            Item::new("item2", "partition0", 6, "zzzz").into(),
        ],
    );
    container.insert(
        "partition1",
        vec![
            Item::new("item0", "partition1", 1, "zzzz").into(),
            Item::new("item1", "partition1", 2, "bbbb").into(),
            Item::new("item2", "partition1", 3, "zzzz").into(),
            Item::new("item3", "partition1", 7, "zzzz").into(),
            Item::new("item4", "partition1", 8, "zzzz").into(),
            Item::new("item5", "partition1", 9, "zzzz").into(),
        ],
    );

    let engine = Engine::new(
        container,
        "SELECT * FROM c",
        QueryPlan {
            partitioned_query_execution_info_version: 1,
            query_info: QueryInfo {
                order_by: vec![SortOrder::Ascending, SortOrder::Descending],
                top: Some(6),
                ..Default::default()
            },
            query_ranges: Vec::new(),
        },
        3,
        QueryResultShape::OrderBy,
    )?;

    // Execute the query, and flatten the response down to just the title for easier comparison.
    let results = engine.execute()?;
    assert_eq!(
        vec![
            EngineResult {
                items: vec![],
                requests: vec![
                    DataRequest::new("partition0", None),
                    DataRequest::new("partition1", None),
                ],
                terminated: false,
            },
            EngineResult {
                items: vec![
                    json!("partition1/item0"),
                    json!("partition0/item0"),
                    json!("partition0/item1"),
                    json!("partition1/item1"),
                    json!("partition1/item2"),
                ],
                requests: vec![DataRequest::new("partition1", Some("3".into())),],
                terminated: false,
            },
            EngineResult {
                items: vec![json!("partition0/item2")],
                requests: vec![],
                terminated: true
            },
        ],
        results
    );

    Ok(())
}

#[test]
pub fn offset_limit() -> Result<(), Box<dyn std::error::Error>> {
    let mut container = Container::new();

    container.insert(
        "partition0",
        vec![
            Item::new("item0", "partition0", 1, "aaaa").into(),
            Item::new("item1", "partition0", 2, "yyyy").into(),
            Item::new("item2", "partition0", 6, "zzzz").into(),
        ],
    );
    container.insert(
        "partition1",
        vec![
            Item::new("item0", "partition1", 1, "zzzz").into(),
            Item::new("item1", "partition1", 2, "bbbb").into(),
            Item::new("item2", "partition1", 3, "zzzz").into(),
            Item::new("item3", "partition1", 7, "zzzz").into(),
            Item::new("item4", "partition1", 8, "zzzz").into(),
            Item::new("item5", "partition1", 9, "zzzz").into(),
        ],
    );

    let engine = Engine::new(
        container,
        "SELECT * FROM c",
        QueryPlan {
            partitioned_query_execution_info_version: 1,
            query_info: QueryInfo {
                order_by: vec![SortOrder::Ascending, SortOrder::Descending],
                offset: Some(3),
                limit: Some(3),
                ..Default::default()
            },
            query_ranges: Vec::new(),
        },
        2, // Really force the engine to make lots of requests.
        QueryResultShape::OrderBy,
    )?;

    // Execute the query, and flatten the response down to just the title for easier comparison.
    let results = engine.execute()?;
    assert_eq!(
        vec![
            EngineResult {
                items: vec![],
                requests: vec![
                    DataRequest::new("partition0", None),
                    DataRequest::new("partition1", None),
                ],
                terminated: false
            },
            EngineResult {
                items: vec![],
                requests: vec![
                    DataRequest::new("partition0", Some("2".into())),
                    DataRequest::new("partition1", Some("2".into()))
                ],
                terminated: false
            },
            EngineResult {
                items: vec![
                    json!("partition1/item1"),
                    json!("partition1/item2"),
                    json!("partition0/item2"),
                ],
                requests: vec![DataRequest::new("partition1", Some("4".into())),],
                terminated: true
            },
        ],
        results
    );

    Ok(())
}
