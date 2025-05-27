// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use std::{path::PathBuf, sync::Arc};

use azure_core::{credentials::Secret, http::TransportOptions};
use azure_data_cosmos::{
    CosmosClient, CosmosClientOptions, PartitionKey, PartitionKeyValue, QueryOptions,
    clients::{ContainerClient, DatabaseClient},
    models::{ContainerProperties, PartitionKeyDefinition},
};
use futures::TryStreamExt;
use serde::Deserialize;
use tracing_subscriber::EnvFilter;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestFileQuery {
    pub name: String,
    pub query: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestFile {
    pub test_data: String,
    pub queries: Vec<TestFileQuery>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestDataFile {
    pub container_properties: ContainerProperties,
    pub data: Vec<serde_json::Value>,
}

// This key is not a secret, it's published in the docs (https://learn.microsoft.com/en-us/azure/cosmos-db/emulator).
const COSMOS_EMULATOR_WELL_KNOWN_KEY: &str =
    "C2y6yDjf5/R+ob0N8A7Cgv30VRDJIWEHLM+4QDU5DE2nQ9nDuVTqobD4b8mGGyPMbIZnqyMsEcaGQy67XIw/Jw==";

fn create_client() -> Result<CosmosClient, azure_core::Error> {
    let endpoint = std::env::var("AZURE_COSMOS_ENDPOINT")
        .unwrap_or_else(|_| "https://localhost:8081".to_string());
    let mut key = std::env::var("AZURE_COSMOS_KEY").unwrap_or_default();
    if key.is_empty() && endpoint == "https://localhost:8081" {
        key = COSMOS_EMULATOR_WELL_KNOWN_KEY.to_string();
    }

    // If we're talking to the emulator, we can disable SSL verification.
    let mut options = CosmosClientOptions::default();
    if endpoint == "https://localhost:8081" {
        let http_client = reqwest::ClientBuilder::new()
            .danger_accept_invalid_certs(true)
            .build()
            .map_err(|e| {
                azure_core::Error::full(
                    azure_core::error::ErrorKind::Other,
                    e,
                    "failed to create HTTP client",
                )
            })?;
        options.client_options.transport = Some(TransportOptions::new(Arc::new(http_client)))
    }

    CosmosClient::with_key(&endpoint, Secret::from(key), Some(options))
}

async fn create_test_container(
    client: &CosmosClient,
    mut properties: ContainerProperties,
    test_id: &str,
    test_name: &str,
) -> Result<(DatabaseClient, ContainerClient), Box<dyn std::error::Error>> {
    let database_name = format!("{}_{}", test_name, test_id);
    client.create_database(&database_name, None).await?;
    tracing::debug!(database_name, "created database");
    let db_client = client.database_client(&database_name);
    properties.id = "TestContainer".into();
    db_client.create_container(properties, None).await?;
    tracing::debug!("created container");
    let container_client = db_client.container_client("TestContainer");
    Ok((db_client, container_client))
}

const BASELINE_QUERIES_DIR: &str = "baselines/queries";
pub async fn run_baseline_test(
    suite_name: &str,
    test_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Enable tracing
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_test_writer()
        .try_init()
        .expect("to successfully initialize tracing");
    let _span = tracing::info_span!("baseline_test", suite_name, test_name).entered();

    // Make the test file path absolute
    let root_dir = {
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR")); // ROOT/azure_data_cosmos_engine/integration-tests
        p.pop(); // ROOT/azure_data_cosmos_engine
        p.pop(); // ROOT
        p
    };
    let test_file_path = root_dir
        .join(BASELINE_QUERIES_DIR)
        .join(format!("{}.json", suite_name));
    let results_file = root_dir
        .join(BASELINE_QUERIES_DIR)
        .join(suite_name)
        .join(format!("{}.results.json", test_name));

    let test_file_dir = {
        let mut p = test_file_path.clone();
        p.pop();
        p
    };

    // Load and parse the test file
    let test_file: TestFile = serde_json::from_str(&std::fs::read_to_string(&test_file_path)?)?;
    let test_query = test_file
        .queries
        .into_iter()
        .find(|q| q.name == test_name)
        .ok_or_else(|| format!("test query '{}' not found", test_name))?;
    tracing::debug!(
        ?test_file_path,
        query = test_query.name,
        "loaded test query file"
    );

    // Load the test data file
    let test_data_file = test_file_dir.join(test_file.test_data);
    let test_data: TestDataFile = serde_json::from_str(&std::fs::read_to_string(&test_data_file)?)?;
    tracing::debug!(?test_data_file, "loaded test data");

    // Create the test database and container
    let test_id = uuid::Uuid::new_v4().simple().to_string();
    let client = create_client()?;
    let (db_client, container_client) = create_test_container(
        &client,
        test_data.container_properties.clone(),
        &test_id,
        test_name,
    )
    .await?;

    // Insert the test data into the container
    {
        let _insert_test_data = tracing::info_span!("insert_test_data");
        tracing::info!("inserting test data");
        for item in test_data.data {
            let key = extract_partition_key(&item, &test_data.container_properties.partition_key)?;
            container_client.create_item(key, &item, None).await?;
        }
        tracing::info!("inserted test data");
    }

    // Now run the requested query
    let items = {
        let _run_query = tracing::info_span!("run_query");
        tracing::info!("running query");

        let options = QueryOptions {
            query_engine: Some(Arc::new(azure_data_cosmos_engine::query::QueryEngine)),
            ..Default::default()
        };
        let pager = container_client.query_items::<serde_json::Value>(
            test_query.query,
            (),
            Some(options),
        )?;
        pager
            .try_collect::<Vec<_>>()
            .await?
            .into_iter()
            .flat_map(|p| p.into_items())
            .map(sanitize_item)
            .collect::<Vec<_>>()
    };

    // Compare the results with the expected results
    let results: Vec<serde_json::Value> =
        serde_json::from_str(&std::fs::read_to_string(&results_file)?)?;
    tracing::info!(?results_file, "loaded expected results");

    for (i, (actual, expected)) in items.iter().zip(results.iter()).enumerate() {
        if actual != expected {
            return Err(format!(
                "item {} was expected to be {:?}, but was {:?}",
                i, expected, actual
            )
            .into());
        }
    }

    // Delete the database
    tracing::info!("deleting database");
    db_client.delete(None).await?;
    tracing::info!("deleted database");

    Ok(())
}

/// Removes system-generated properties from the item.
fn sanitize_item(item: serde_json::Value) -> serde_json::Value {
    let serde_json::Value::Object(mut map) = item else {
        return item;
    };

    map.remove("_rid");
    map.remove("_self");
    map.remove("_etag");
    map.remove("_ts");

    serde_json::Value::Object(map)
}

fn extract_partition_key(
    item: &serde_json::Value,
    partition_key: &PartitionKeyDefinition,
) -> Result<PartitionKey, Box<dyn std::error::Error>> {
    let mut values = Vec::new();
    for path in &partition_key.paths {
        values.push(extract_single_partition_key(item, path)?);
    }

    // TODO: Replace with PartitionKey::from when https://github.com/Azure/azure-sdk-for-rust/issues/2612 is fixed.
    match values.len() {
        0 => Err("partition key must have at least one path".into()),
        1 => Ok(PartitionKey::from(values[0].clone())),
        _ => Err("partition key must have exactly one path".into()), // TODO: We can support HPK once the bug above is fixed.
    }
}

fn extract_single_partition_key(
    item: &serde_json::Value,
    mut path: &str,
) -> Result<PartitionKeyValue, Box<dyn std::error::Error>> {
    let original_path = path;
    if !path.starts_with('/') {
        return Err(format!(
            "partition key path '{}' does not start with '/'",
            original_path
        )
        .into());
    }

    path = &path[1..];

    if path.contains('/') {
        return Err(format!(
            "partition key path '{}' references a nested property, which is not supported",
            original_path
        )
        .into());
    }

    let serde_json::Value::Object(map) = item else {
        return Err("items must be JSON objects".into());
    };

    let value = map
        .get(path)
        .ok_or_else(|| format!("partition key path '{}' not found", original_path))?;
    match value {
        serde_json::Value::String(s) => Ok(s.into()),
        _ => Err(format!(
            "partition key path '{}' must be a string, but found '{:?}'",
            original_path, value
        )
        .into()),
    }
}

macro_rules! baseline_tests {
    (
        $(
            $testsuite:ident {
                $(
                    $test:ident,
                )*
            }
        ),*
    ) => {
        $(
            mod $testsuite {
                $(
                    #[tokio::test]
                    async fn $test() -> Result<(), Box<dyn std::error::Error>> {
                        let suite_name = stringify!($testsuite);
                        let test_name = stringify!($test);
                        $crate::runner::run_baseline_test(suite_name, test_name).await
                    }
                )*
            }
        )*
    };
}
