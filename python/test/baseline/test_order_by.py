from typing import Optional

import os
import pytest

import azure.cosmos
import azure_cosmoscx

baseline_test_enabled = os.getenv(
    "COSMOSCX_BASELINE_TEST", "false").lower() == "true"

if not baseline_test_enabled:
    pytest.skip(reason="COSMOSCX_BASELINE_TEST is not 'true'",
                allow_module_level=True)

azure_cosmoscx.enable_tracing()

endpoint = os.getenv("COSMOSCX_ENDPOINT", "https://localhost:8081")
key = os.getenv(
    "COSMOSCX_KEY", "C2y6yDjf5/R+ob0N8A7Cgv30VRDJIWEHLM+4QDU5DE2nQ9nDuVTqobD4b8mGGyPMbIZnqyMsEcaGQy67XIw/Jw==")
databaseName = os.getenv("COSMOSCX_DATABASE", "TestDB")
containerName = os.getenv("COSMOSCX_CONTAINER", "TestContainer")


class BaselineTestFixture:
    def __init__(self):
        self.python_client = azure.cosmos.CosmosClient(
            endpoint, key, connection_verify=False)
        self.python_db = self.python_client.get_database_client(databaseName)
        self.python_container = self.python_db.get_container_client(
            containerName)

        self.cx_client = azure.cosmos.CosmosClient(
            endpoint, key, connection_verify=False)
        self.cx_db = self.cx_client.get_database_client(databaseName)
        self.cx_container = self.cx_db.get_container_client(containerName)

    def run_baseline_test(self, query: str):
        python_items = list(self.python_container.query_items(
            query, enable_cross_partition_query=True))
        cx_items = list(self.cx_container.query_items(
            query, enable_cross_partition_query=True))
        assert python_items == cx_items


@pytest.fixture
def baseline_test_fixture():
    return BaselineTestFixture()


def test_ascending_string_order_by(baseline_test_fixture):
    baseline_test_fixture.run_baseline_test("SELECT * FROM c ORDER BY c.name")


def test_descending_string_order_by(baseline_test_fixture):
    baseline_test_fixture.run_baseline_test(
        "SELECT * FROM c ORDER BY c.name DESC")
