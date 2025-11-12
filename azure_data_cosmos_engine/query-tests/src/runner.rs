// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use azure_core::{credentials::Secret, http::Transport};
use azure_data_cosmos::{
    clients::{ContainerClient, DatabaseClient},
    models::{ContainerProperties, PartitionKeyDefinition, ThroughputProperties},
    CosmosClient, CosmosClientOptions, CreateContainerOptions, PartitionKey, PartitionKeyValue,
    Query, QueryOptions,
};
use futures::TryStreamExt;
use serde::Deserialize;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestFileQuery {
    pub name: String,
    pub query: String,
    pub container: String,
    #[serde(default)]
    pub parameters: HashMap<String, serde_json::Value>,

    #[serde(default)]
    pub validators: HashMap<String, String>,
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
    pub containers: Vec<ContainerProperties>,
    pub data: Vec<HashMap<String, serde_json::Value>>,
    #[serde(default)]
    pub parameters: HashMap<String, serde_json::Value>,
}

struct ValidationError {
    item: usize,
    property_name: String,
    message: String,
    expected: serde_json::Value,
    actual: serde_json::Value,
}

// f64::EPSILON isn't sufficient for our needs here, we need a bit more leeway.
// Since we're comparing between different SDKs, AND dealing with JSON serialization, EPSILON is just too tight.
const ALLOWED_FLOAT_ERROR: f64 = 1e-6;

fn get_validator<'a>(validators: &'a HashMap<String, String>, property_name: &str) -> &'a str {
    if let Some(validator) = validators.get(property_name) {
        return validator;
    }

    match property_name {
        "_etag" => "ignore",
        "_rid" => "ignore",
        "_self" => "ignore",
        "_ts" => "ignore",
        "_attachments" => "ignore",
        _ => "equal",
    }
}

fn validator_ordered(
    property_name: &str,
    expected_items: &[serde_json::Value],
    actual_items: &[serde_json::Value],
    ascending: bool,
) -> Vec<ValidationError> {
    if actual_items.len() < 2 {
        return Vec::new(); // No errors if there are fewer than 2 items.
    }
    for i in 1..actual_items.len() {
        let left = &actual_items[i - 1];
        let right = &actual_items[i];

        let comparison_valid = match (left, right) {
            (serde_json::Value::Number(left_num), serde_json::Value::Number(right_num)) => {
                if ascending {
                    left_num.as_f64() <= right_num.as_f64()
                } else {
                    left_num.as_f64() >= right_num.as_f64()
                }
            }
            (serde_json::Value::String(left_str), serde_json::Value::String(right_str)) => {
                if ascending {
                    left_str <= right_str
                } else {
                    left_str >= right_str
                }
            }
            _ => {
                return vec![ValidationError {
                    item: i,
                    property_name: property_name.to_string(),
                    message: format!(
                        "unsupported comparison for '{}' in items at index {} and {}: {} and {}",
                        property_name,
                        i - 1,
                        i,
                        left,
                        right
                    ),
                    expected: left.clone(),
                    actual: right.clone(),
                }];
            }
        };

        if !comparison_valid {
            return vec![ValidationError {
                item: i,
                property_name: property_name.to_string(),
                message: format!(
                    "items are not ordered {} by '{}': {} and {}",
                    if ascending { "ascending" } else { "descending" },
                    property_name,
                    left,
                    right,
                ),
                expected: expected_items[i - 1].clone(),
                actual: actual_items[i].clone(),
            }];
        }
    }
    Vec::new()
}

fn validate_property(
    validator: &str,
    property_name: &str,
    expected_items: &[serde_json::Value],
    actual_items: &[serde_json::Value],
) -> Vec<ValidationError> {
    match validator {
        "ignore" => Vec::new(),
        "orderedDescending" => {
            validator_ordered(property_name, expected_items, actual_items, false)
        }
        "orderedAscending" => validator_ordered(property_name, expected_items, actual_items, true),
        "equal" => validator_equal(property_name, expected_items, actual_items),
        x => panic!("unknown validator '{x}' for property '{property_name}'"),
    }
}

fn validator_equal(
    property_name: &str,
    expected_items: &[serde_json::Value],
    actual_items: &[serde_json::Value],
) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    for (id, (expected, actual)) in expected_items.iter().zip(actual_items.iter()).enumerate() {
        if expected != actual {
            errors.push(ValidationError {
                item: id,
                property_name: property_name.to_string(),
                message: format!(
                    "expected '{property_name}' to be equal, but found different values"
                ),
                expected: expected.clone(),
                actual: actual.clone(),
            });
        }
    }
    errors
}

// This key is not a secret, it's published in the docs (https://learn.microsoft.com/en-us/azure/cosmos-db/emulator).
const COSMOS_EMULATOR_WELL_KNOWN_KEY: &str =
    "C2y6yDjf5/R+ob0N8A7Cgv30VRDJIWEHLM+4QDU5DE2nQ9nDuVTqobD4b8mGGyPMbIZnqyMsEcaGQy67XIw/Jw==";

const THROUGHPUT_FOR_TWO_PARTITIONS: usize = 40_000;

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
                azure_core::Error::with_error(
                    azure_core::error::ErrorKind::Other,
                    e,
                    "failed to create HTTP client",
                )
            })?;
        options.client_options.transport = Some(Transport::new(Arc::new(http_client)))
    }

    CosmosClient::with_key(&endpoint, Secret::from(key), Some(options))
}

async fn create_test_container(
    client: &CosmosClient,
    properties: ContainerProperties,
    test_id: &str,
    test_name: &str,
) -> Result<(DatabaseClient, ContainerClient), Box<dyn std::error::Error>> {
    let database_name = format!("{test_name}_{test_id}");
    client.create_database(&database_name, None).await?;
    tracing::debug!(database_name, "created database");
    let db_client = client.database_client(&database_name);
    let id = properties.id.clone();
    db_client
        .create_container(
            properties,
            Some(CreateContainerOptions {
                throughput: Some(ThroughputProperties::manual(THROUGHPUT_FOR_TWO_PARTITIONS)),
                ..Default::default()
            }),
        )
        .await?;
    tracing::debug!("created container");
    let container_client = db_client.container_client(&id);
    Ok((db_client, container_client))
}

static TRACING_SUBSCRIBER_INIT: std::sync::Once = std::sync::Once::new();

const BASELINE_QUERIES_DIR: &str = "baselines/queries";
pub async fn run_baseline_test(
    suite_name: &str,
    test_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Enable tracing
    TRACING_SUBSCRIBER_INIT.call_once(|| {
        tracing_subscriber::fmt()
            .with_env_filter(
                EnvFilter::builder()
                    .with_default_directive(LevelFilter::WARN.into()) // Log errors by default
                    .from_env_lossy(),
            )
            .with_test_writer()
            .try_init()
            .expect("to successfully initialize tracing");
    });
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
        .join(format!("{suite_name}.json"));
    let results_file = root_dir
        .join(BASELINE_QUERIES_DIR)
        .join(suite_name)
        .join(format!("{test_name}.results.json"));

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
        .ok_or_else(|| format!("test query '{test_name}' not found"))?;
    tracing::debug!(
        ?test_file_path,
        query = test_query.name,
        "loaded test query file"
    );

    // Load the test data file
    let test_data_file = test_file_dir.join(test_file.test_data);
    let test_data: TestDataFile = serde_json::from_str(&std::fs::read_to_string(&test_data_file)?)?;
    tracing::debug!(?test_data_file, "loaded test data");

    // Identify which container to use for the test
    let test_container_properties = test_data
        .containers
        .iter()
        .find(|c| c.id == test_query.container)
        .ok_or_else(|| {
            format!(
                "test query '{}' references container '{}', but that container was not found in the test data
                file",
                test_query.name, test_query.container
            )
        })?;

    // Create the test database and container
    let test_id = uuid::Uuid::new_v4().simple().to_string();
    let client = create_client()?;
    let (db_client, container_client) = create_test_container(
        &client,
        test_container_properties.clone(),
        &test_id,
        test_name,
    )
    .await?;

    // Insert the test data into the container
    {
        let _insert_test_data = tracing::info_span!("insert_test_data");
        tracing::info!("inserting test data");
        for item in test_data.data {
            let key = extract_partition_key(&item, &test_container_properties.partition_key)?;
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
        let mut query = Query::from(test_query.query);
        for (name, value) in test_query.parameters {
            query = query.with_parameter(format!("@{name}"), value)?;
        }
        for (name, value) in test_data.parameters {
            query = query.with_parameter(format!("@testData_{name}"), value)?;
        }

        // Some simple retry logic because the emulator can be flaky on CI (because we're running on slower machines).
        let mut retry_count = 0;
        const MAX_RETRIES: usize = 3;
        loop {
            let pager = container_client.query_items::<serde_json::Value>(
                query.clone(),
                (),
                Some(options.clone()),
            )?;
            let collect_result = pager.try_collect::<Vec<_>>().await;
            match collect_result {
                Ok(items) => break items,
                Err(e) if e.http_status() == Some(azure_core::http::StatusCode::RequestTimeout) => {
                    tracing::warn!(?e, "query failed, retrying");
                    retry_count += 1;
                    if retry_count == MAX_RETRIES {
                        return Err(format!("query failed after {MAX_RETRIES} retries: {e}").into());
                    }
                }
                Err(e) => {
                    tracing::error!(?e, "query failed for non-retryable reason");
                    return Err(e.into());
                }
            }
        }
    };

    // Compare the results with the expected results
    let results: Vec<serde_json::Value> =
        serde_json::from_str(&std::fs::read_to_string(&results_file)?)?;
    tracing::info!(?results_file, "loaded expected results");

    if items.len() != results.len() {
        panic!(
            "query returned {} items, but expected {} items",
            items.len(),
            results.len()
        );
    }

    // If the first expected item is an object, we do per-property validation.
    let errors = if let Some(serde_json::Value::Object(_)) = results.first() {
        validate_with_validators(&test_query.validators, &items, &results)?
    } else {
        // Otherwise, we just compare the values directly.
        let mut errors = Vec::new();
        for (i, (actual, expected)) in items.iter().zip(results.iter()).enumerate() {
            // Take some extra care when comparing floating point numbers.
            // There can be precision issues, and we're comparing against JSON text generated by the .NET SDK

            // "is_f64" is not guaranteed to be false when the number is an integer.
            // See https://docs.rs/serde_json/latest/serde_json/struct.Number.html#method.is_f64

            if !actual.is_i64() && !expected.is_i64() && !actual.is_u64() && !expected.is_u64() {
                // This is definitely a floating point number.
                // So, rather than true equality, we compute the difference and ensure it's within the machine limit for floating point precision.
                let actual_f = actual
                    .as_f64()
                    .ok_or_else(|| format!("failed to convert actual number '{actual}' to f64"))?;
                let expected_f = expected.as_f64().ok_or_else(|| {
                    format!("failed to convert expected number '{expected}' to f64")
                })?;
                let delta = (actual_f - expected_f).abs();
                if delta > ALLOWED_FLOAT_ERROR {
                    errors.push(ValidationError {
                        item: i,
                        property_name: "<item>".to_string(),
                        message: format!(
                            "floating point numbers do not match within precision value {}: delta is {}",
                            ALLOWED_FLOAT_ERROR,
                            delta
                        ),
                        expected: expected.clone(),
                        actual: actual.clone(),
                    });
                }
                continue;
            }

            // If we got here, we either have an integer or non-number, so we can do a direct comparison.
            if actual != expected {
                errors.push(ValidationError {
                    item: i,
                    property_name: "<item>".to_string(),
                    message: "items do not match".to_string(),
                    expected: expected.clone(),
                    actual: actual.clone(),
                });
            }
        }
        errors
    };

    if !errors.is_empty() {
        for error in &errors {
            tracing::error!(
                item = error.item,
                property = error.property_name,
                expected = ?error.expected,
                actual = ?error.actual,
                "validation error: {}",
                error.message
            );
        }
        return Err(format!("validation failed with {} errors", errors.len()).into());
    }

    // Delete the database
    tracing::info!("deleting database");
    db_client.delete(None).await?;
    tracing::info!("deleted database");

    Ok(())
}

fn validate_with_validators(
    validators: &HashMap<String, String>,
    actual_items: &[serde_json::Value],
    expected_items: &[serde_json::Value],
) -> Result<Vec<ValidationError>, Box<dyn std::error::Error>> {
    let mut all_errors = Vec::new();

    // For each property in the expected items, validate according to the specified validator.
    if let Some(expected_first) = expected_items.first() {
        if let serde_json::Value::Object(expected_map) = expected_first {
            for property_name in expected_map.keys() {
                let validator = get_validator(validators, property_name);
                let expected_property_values: Vec<serde_json::Value> = expected_items
                    .iter()
                    .map(|item| {
                        item.get(property_name)
                            .cloned()
                            .unwrap_or(serde_json::Value::Null)
                    })
                    .collect();
                let actual_property_values: Vec<serde_json::Value> = actual_items
                    .iter()
                    .map(|item| {
                        item.get(property_name)
                            .cloned()
                            .unwrap_or(serde_json::Value::Null)
                    })
                    .collect();

                let errors = validate_property(
                    validator,
                    property_name,
                    &expected_property_values,
                    &actual_property_values,
                );
                all_errors.extend(errors);
            }
        } else {
            return Err("expected items are not JSON objects".into());
        }
    }

    Ok(all_errors)
}

fn extract_partition_key(
    item: &HashMap<String, serde_json::Value>,
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
    item: &HashMap<String, serde_json::Value>,
    mut path: &str,
) -> Result<PartitionKeyValue, Box<dyn std::error::Error>> {
    let original_path = path;
    if !path.starts_with('/') {
        return Err(format!("partition key path '{original_path}' does not start with '/'").into());
    }

    path = &path[1..];

    if path.contains('/') {
        return Err(format!(
            "partition key path '{original_path}' references a nested property, which is not supported"
        )
        .into());
    }

    let value = item
        .get(path)
        .ok_or_else(|| format!("partition key path '{original_path}' not found"))?;
    match value {
        serde_json::Value::String(s) => Ok(s.into()),
        _ => Err(format!(
            "partition key path '{original_path}' must be a string, but found '{value:?}'"
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
            },
        )*
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
