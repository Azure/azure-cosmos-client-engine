// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use std::vec;

use azure_data_cosmos_engine::query::{DataRequest, ItemIdentity, QueryResult};

use pretty_assertions::assert_eq;

use mock_engine::{Container, Engine};

use crate::mock_engine::EngineResult;

mod mock_engine;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
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
        let value = serde_json::value::to_raw_value(&item).unwrap();
        QueryResult::RawPayload(value)
    }
}

#[test]
pub fn read_many() -> Result<(), Box<dyn std::error::Error>> {
    let mut container = Container::new();

    let partition_0_items = vec![
        Item::new("item0", "even"),
        Item::new("item2", "even"),
        Item::new("item4", "even"),
        Item::new("item6", "even"),
        Item::new("item8", "even"),
        Item::new("item10", "even"),
    ];
    let partition_1_items = vec![
        Item::new("item1", "odd"),
        Item::new("item3", "odd"),
        Item::new("item5", "odd"),
        Item::new("item7", "odd"),
        Item::new("item9", "odd"),
        Item::new("item11", "odd"),
    ];

    container.insert(
        "even",
        partition_0_items
            .into_iter()
            .map(|item| item.into())
            .collect::<Vec<_>>(),
    );
    container.insert(
        "odd",
        partition_1_items
            .into_iter()
            .map(|item| item.into())
            .collect::<Vec<_>>(),
    );
    let item_identities = vec![
        ItemIdentity::new("item0", "even"),
        ItemIdentity::new("item1", "odd"),
        ItemIdentity::new("item2", "even"),
        ItemIdentity::new("item3", "odd"),
        ItemIdentity::new("item4", "even"),
        ItemIdentity::new("item5", "odd"),
        ItemIdentity::new("item6", "even"),
        ItemIdentity::new("item7", "odd"),
        ItemIdentity::new("item8", "even"),
        ItemIdentity::new("item9", "odd"),
        ItemIdentity::new("item10", "even"),
        ItemIdentity::new("item11", "odd"),
    ];

    let expected_even_query = "SELECT * FROM c WHERE ( (c.id='item0' AND c.pk='even') OR (c.id='item2' AND c.pk='even') OR (c.id='item4' AND c.pk='even') OR (c.id='item6' AND c.pk='even') OR (c.id='item8' AND c.pk='even') OR (c.id='item10' AND c.pk='even') )";
    let expected_odd_query = "SELECT * FROM c WHERE ( (c.id='item1' AND c.pk='odd') OR (c.id='item3' AND c.pk='odd') OR (c.id='item5' AND c.pk='odd') OR (c.id='item7' AND c.pk='odd') OR (c.id='item9' AND c.pk='odd') OR (c.id='item11' AND c.pk='odd') )";

    let engine = Engine::for_read_many(container, item_identities, 10)?;

    // We should see the first call return all of the relevant query requests
    // We should see the second call return all of the relevant items across partitions
    let results = engine.execute()?;
    assert_eq!(
        vec![
            EngineResult {
                items: vec![],
                // Note: The order of requests depends on hash distribution - "odd" comes first based on partition ranges
                requests: vec![
                    DataRequest::with_query(0, "even", None, expected_even_query.to_string(), true),
                    DataRequest::with_query(1, "odd", None, expected_odd_query.to_string(), true),
                ],
                terminated: false,
            },
            EngineResult {
                items: vec![
                    // All even partition items
                    serde_json::json!({"id": "item0", "partition_key": "even", "title": "even/item0"}),
                    serde_json::json!({"id": "item2", "partition_key": "even", "title": "even/item2"}),
                    serde_json::json!({"id": "item4", "partition_key": "even", "title": "even/item4"}),
                    serde_json::json!({"id": "item6", "partition_key": "even", "title": "even/item6"}),
                    serde_json::json!({"id": "item8", "partition_key": "even", "title": "even/item8"}),
                    serde_json::json!({"id": "item10", "partition_key": "even", "title": "even/item10"}),
                    // All odd partition items
                    serde_json::json!({"id": "item1", "partition_key": "odd", "title": "odd/item1"}),
                    serde_json::json!({"id": "item3", "partition_key": "odd", "title": "odd/item3"}),
                    serde_json::json!({"id": "item5", "partition_key": "odd", "title": "odd/item5"}),
                    serde_json::json!({"id": "item7", "partition_key": "odd", "title": "odd/item7"}),
                    serde_json::json!({"id": "item9", "partition_key": "odd", "title": "odd/item9"}),
                    serde_json::json!({"id": "item11", "partition_key": "odd", "title": "odd/item11"}),
                ],
                requests: vec![],
                terminated: true,
            },
        ],
        results
    );

    Ok(())
}
