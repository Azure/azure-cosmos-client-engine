use std::vec;

use azure_data_cosmos_client_engine::query::{QueryPlan, QueryResult};
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
    pub fn new(
        id: impl Into<String>,
        partition_key: impl Into<String>,
        title: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            partition_key: partition_key.into(),
            title: title.into(),
        }
    }
}

impl From<Item> for QueryResult<Item> {
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
            Item::new("item0", "partition0", "partition0 / item0").into(),
            Item::new("item1", "partition0", "partition0 / item1").into(),
            Item::new("item2", "partition0", "partition0 / item2").into(),
            Item::new("item3", "partition0", "partition0 / item3").into(),
            Item::new("item4", "partition0", "partition0 / item4").into(),
            Item::new("item5", "partition0", "partition0 / item5").into(),
        ],
    );
    container.insert(
        "partition1",
        vec![
            Item::new("item0", "partition1", "partition1 / item0").into(),
            Item::new("item1", "partition1", "partition1 / item1").into(),
            Item::new("item2", "partition1", "partition1 / item2").into(),
            Item::new("item3", "partition1", "partition1 / item3").into(),
            Item::new("item4", "partition1", "partition1 / item4").into(),
            Item::new("item5", "partition1", "partition1 / item5").into(),
        ],
    );

    let engine = Engine::new(
        container,
        QueryPlan {
            partitioned_query_execution_info_version: 1,
            query_info: Default::default(),
            query_ranges: Vec::new(),
        },
        3,
    );

    let results = engine.execute()?;
    assert_eq!(
        vec![
            vec![
                Item::new("item0", "partition0", "partition0 / item0"),
                Item::new("item1", "partition0", "partition0 / item1"),
                Item::new("item2", "partition0", "partition0 / item2"),
            ],
            vec![
                Item::new("item3", "partition0", "partition0 / item3"),
                Item::new("item4", "partition0", "partition0 / item4"),
                Item::new("item5", "partition0", "partition0 / item5"),
                // NOTE: We expect no page gap here, because the REQUEST page size is 3, which means we'll get no more than 3 items per partition.
                Item::new("item0", "partition1", "partition1 / item0"),
                Item::new("item1", "partition1", "partition1 / item1"),
                Item::new("item2", "partition1", "partition1 / item2"),
            ],
            vec![
                Item::new("item3", "partition1", "partition1 / item3"),
                Item::new("item4", "partition1", "partition1 / item4"),
                Item::new("item5", "partition1", "partition1 / item5"),
            ],
        ],
        results
    );

    Ok(())
}
