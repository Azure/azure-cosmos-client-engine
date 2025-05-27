// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use std::vec;

use azure_data_cosmos_engine::query::{
    DataRequest, JsonQueryClauseItem, PipelineResponse, QueryPlan, QueryResult,
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
    )?;

    // Execute the query, and flatten the response down to just the title for easier comparison.
    let results = engine.execute()?;
    let titles = results
        .into_iter()
        .map(|response| response.map_items(|item| item.title))
        .collect::<Vec<_>>();
    assert_eq!(
        vec![
            PipelineResponse {
                items: vec![],
                requests: vec![
                    DataRequest::new("partition0", None),
                    DataRequest::new("partition1", None),
                ],
                terminated: false,
            },
            PipelineResponse {
                items: vec![
                    "partition0/item0".to_string(),
                    "partition0/item1".to_string(),
                    "partition0/item2".to_string(),
                ],
                requests: vec![
                    DataRequest::new("partition0", Some("3".into())),
                    DataRequest::new("partition1", Some("3".into()))
                ],
                terminated: false,
            },
            PipelineResponse {
                items: vec![
                    "partition0/item3".to_string(),
                    "partition0/item4".to_string(),
                    "partition0/item5".to_string(),
                    "partition1/item0".to_string(),
                    "partition1/item1".to_string(),
                    "partition1/item2".to_string(),
                    "partition1/item3".to_string(),
                    "partition1/item4".to_string(),
                    "partition1/item5".to_string(),
                ],
                requests: vec![],
                terminated: true,
            },
        ],
        titles
    );

    Ok(())
}
