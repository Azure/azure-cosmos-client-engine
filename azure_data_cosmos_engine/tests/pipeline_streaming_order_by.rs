// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use std::vec;

use azure_data_cosmos_engine::query::{
    DataRequest, QueryClauseItem, QueryInfo, QueryPlan, QueryResult, SortOrder,
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
        QueryResult::OrderBy {
            order_by_items: vec![sort0, sort1],
            payload: raw,
        }
    }
}

#[test]
pub fn streaming_order_by() -> Result<(), Box<dyn std::error::Error>> {
    // We have fairly deep comparison tests in the query_result module itself, so we're not worried about comparing literally every combination of JSON value.

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
            query_info: Some(QueryInfo {
                order_by: vec![SortOrder::Ascending, SortOrder::Descending],
                ..Default::default()
            }),
            ..Default::default()
        },
        3,
    )?;

    // Execute the query, and flatten the response down to just the title for easier comparison.
    let results = engine.execute()?;
    assert_eq!(
        vec![
            EngineResult {
                items: vec![],
                requests: vec![
                    DataRequest::new(0, "partition0", None, None),
                    DataRequest::new(0, "partition1", None, None),
                ],
                terminated: false
            },
            EngineResult {
                items: vec![
                    json!("partition1/item0"),
                    json!("partition0/item0"),
                    json!("partition0/item1"),
                    json!("partition1/item1"),
                    json!("partition1/item2"),
                ],
                requests: vec![DataRequest::new(1, "partition1", Some("3".into()), None),],
                terminated: false
            },
            EngineResult {
                items: vec![
                    json!("partition0/item2"),
                    json!("partition1/item3"),
                    json!("partition1/item4"),
                    json!("partition1/item5"),
                ],
                requests: vec![],
                terminated: true
            },
        ],
        results
    );

    Ok(())
}
