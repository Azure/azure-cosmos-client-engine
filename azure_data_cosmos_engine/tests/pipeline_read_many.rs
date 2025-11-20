// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use std::vec;

use azure_data_cosmos_engine::query::{DataRequest, ItemIdentity, QueryResult};

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
        partition_0_items.into_iter().map(|item| item.into()).collect::<Vec<_>>(),
    );
    container.insert(
        "odd",
        partition_1_items.into_iter().map(|item| item.into()).collect::<Vec<_>>(),
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

    let engine = Engine::for_read_many(
        container,
        item_identities,
        10,
    )?;

    // We should see the first call return all of the relevant query requests
    let results = engine.execute()?;
    assert_eq!(
        vec![
            EngineResult {
                items: vec![],
                requests: vec![DataRequest::new(0, "even", None, Some(expected_even_query.to_string())),
                               DataRequest::new(1, "odd", None, Some(expected_odd_query.to_string())),],
                terminated: false,
            },
            EngineResult {
                items: vec![],
                requests: vec![],
                terminated: true,
            },
        ],
        results
    );

    Ok(())
}
