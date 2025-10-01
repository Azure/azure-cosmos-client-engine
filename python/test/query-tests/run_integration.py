# Copyright (c) Microsoft Corporation.
# Licensed under the MIT License.

import json
import pathlib
import uuid

from azure.cosmos import CosmosClient, PartitionKey, ContainerProxy

from typing import Callable, Dict, TypedDict, Any, List, Tuple, Set

import azure_cosmoscx
from test.test_config import TestConfig

CONTAINERS = "containers"
DATA = "data"
NAME = "name"
TEST_DATA = "testData"
QUERIES = "queries"
QUERY = "query"
ID = "id"
PARTITION_KEY = "partitionKey"
PATHS = "paths"
RESULTS_SUFFIX = ".results.json"

class TestData(TypedDict):
    containers: Dict[str, Any]
    data: List[Dict[str, Any]]


class QuerySpec(TypedDict):
    name: str
    query: str
    container: str
    validators: Dict[str, Any]


class QuerySet(TypedDict):
    name: str
    testData: str
    queries: List[QuerySpec]

def _run_with_resources(
        test_data: TestData,
        fn: Callable[[ContainerProxy], None],
        unique_name: str
) -> None:
    client = CosmosClient(url=TestConfig.host, credential=TestConfig.masterKey, query_engine=azure_cosmoscx.QueryEngine())
    db = client.create_database_if_not_exists(id=unique_name)
    try:
        for container_def in test_data[CONTAINERS]:
        
            pk_paths: list[str] = container_def[PARTITION_KEY][PATHS]
            pk = PartitionKey(path=pk_paths[0])  # single-path only
            container = db.create_container_if_not_exists(
                id=container_def[ID],
                partition_key=pk,
                offer_throughput=40000
            )

            # insert documents
            for item in test_data[DATA]:
                container.create_item(body=item)

            # hand control to the caller
            fn(container)
    finally:
        client.delete_database(unique_name)

# gets the information for the query being tested and sample data to insert to container from a file
def _load_query_context(query_path: pathlib.Path) -> Tuple[QuerySet, TestData, pathlib.Path]:
    with query_path.open("rb") as fh:
        query_spec: QuerySet = json.load(fh)

    test_path = str(query_path) + "/../" + query_spec[TEST_DATA]

    test_file = pathlib.Path(test_path).resolve()

    with test_file.open("rb") as fh:
        test_data: TestData = json.load(fh)

    return query_spec, test_data, query_path

def validate_results(expected: dict[str, Any], actual: dict[str, Any], ignored_keys: Set[str]) -> None:

    # removes some metadata keys that are not relevant for testing
    for key in ignored_keys:
        actual.pop(key, None)
        expected.pop(key, None)

    assert expected == actual

def _run_single_query(expected: list[dict[str, Any]], query: QuerySpec, container) -> None:
    iterator = container.query_items(
        query=query[QUERY],
        enable_cross_partition_query=True
    )
    results = list(iterator)
    ignored_keys = {"_rid", "_self", "_etag", "_attachments", "_ts"}
    for i, item in enumerate(results):
        validate_results(expected[i], item, ignored_keys)
    assert len(results) == len(expected)

def run_integration_test(query_set_path: str) -> None:
    full_path = pathlib.Path(query_set_path).resolve()

    query_set, test_data, query_path = _load_query_context(full_path)
    unique_name = f"it_{query_set[NAME]}_{uuid.uuid4()}"

    # gets expected results from file and runs the queries to be tested
    def _runner(container: ContainerProxy) -> None:
        for query in query_set[QUERIES]:
            res_file = query_path.parent / f"{query_set[NAME]}/{query[NAME]}{RESULTS_SUFFIX}"
            with res_file.open("rb") as fh:
                expected = json.load(fh)
            _run_single_query(expected, query, container)
            print(f"✓ {query[NAME]}")

    _run_with_resources(test_data, _runner, unique_name)
