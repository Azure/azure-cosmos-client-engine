use std::vec;

use azure_data_cosmos_client_engine::query::{
    DataRequest, PipelineResponse, QueryClauseItem, QueryInfo, QueryPlan, QueryResult, SortOrder,
};
use pretty_assertions::assert_eq;

use mock_engine::{Container, Engine};

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

impl From<Item> for QueryResult<Item> {
    fn from(item: Item) -> Self {
        let sort0: QueryClauseItem = QueryClauseItem {
            item: Some(serde_json::Value::Number(serde_json::Number::from(
                item.sort0,
            ))),
        };
        let sort1: QueryClauseItem = QueryClauseItem {
            item: Some(serde_json::Value::String(item.sort1.clone())),
        };
        QueryResult::new(vec![], vec![sort0, sort1], item)
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
    )?;

    // Execute the query, and flatten the response down to just the title for easier comparison.
    let results = engine.execute()?;
    let titles = results
        .into_iter()
        .map(|response| PipelineResponse {
            batch: response
                .batch
                .into_iter()
                .map(|item| item.title.clone())
                .collect::<Vec<_>>(),
            requests: response.requests,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        vec![
            PipelineResponse {
                batch: vec![],
                requests: vec![
                    DataRequest::new("partition0", None),
                    DataRequest::new("partition1", None),
                ]
            },
            PipelineResponse {
                batch: vec![
                    "partition1/item0".to_string(),
                    "partition0/item0".to_string(),
                    "partition0/item1".to_string(),
                    "partition1/item1".to_string(),
                    "partition1/item2".to_string(),
                ],
                requests: vec![DataRequest::new("partition1", Some("3".into())),],
            },
            PipelineResponse {
                batch: vec!["partition0/item2".to_string(),],
                requests: vec![],
            },
        ],
        titles
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
    )?;

    // Execute the query, and flatten the response down to just the title for easier comparison.
    let results = engine.execute()?;
    let titles = results
        .into_iter()
        .map(|response| PipelineResponse {
            batch: response
                .batch
                .into_iter()
                .map(|item| item.title.clone())
                .collect::<Vec<_>>(),
            requests: response.requests,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        vec![
            PipelineResponse {
                batch: vec![],
                requests: vec![
                    DataRequest::new("partition0", None),
                    DataRequest::new("partition1", None),
                ]
            },
            PipelineResponse {
                batch: vec![],
                requests: vec![DataRequest::new("partition0", Some("2".into())),]
            },
            PipelineResponse {
                batch: vec!["partition1/item1".to_string(),],
                requests: vec![DataRequest::new("partition1", Some("2".into())),]
            },
            PipelineResponse {
                batch: vec![
                    "partition1/item2".to_string(),
                    "partition0/item2".to_string(),
                ],
                requests: vec![]
            },
        ],
        titles
    );

    Ok(())
}
