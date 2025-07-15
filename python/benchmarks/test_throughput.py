# Copyright (c) Microsoft Corporation.
# Licensed under the MIT License.

"""
Throughput benchmarks for the Azure Cosmos DB Python client engine.

This module benchmarks the raw throughput of the query pipeline under various conditions.
The goal of these benchmarks is to provide a baseline for comparing the performance impact
that arises when we wrap the Rust engine in Python, and to compare against the Rust and Go benchmarks.
"""

import json
import pytest
import azure_cosmoscx
import time
from typing import Dict, List, Any, Callable
from .config import PARTITION_COUNT, PAGE_SIZE
from .utils import (
    BenchmarkItem,
    create_partition_data,
    create_partition_key_ranges,
    unordered_item_formatter,
    ordered_item_formatter,
    create_simple_query_plan,
    create_ordered_query_plan
)


class BenchmarkScenario:
    """Benchmark scenario configuration."""

    def __init__(
        self,
        name: str,
        query_sql: str,
        query_plan_fn: Callable[[], Dict[str, Any]],
        item_formatter_fn: Callable[[BenchmarkItem], str]
    ):
        self.name = name
        self.query_sql = query_sql
        self.query_plan_fn = query_plan_fn
        self.item_formatter_fn = item_formatter_fn


def fulfill_data_requests(
    pipeline,
    requests: List[Any],
    partition_data: Dict[str, List[BenchmarkItem]],
    scenario: BenchmarkScenario
) -> None:
    """Fulfill data requests from the pipeline."""
    for request in requests:
        pkrange_id = request.pkrange_id
        if pkrange_id in partition_data:
            # Calculate which items to return based on continuation
            start_index = 0
            if request.continuation:
                try:
                    start_index = int(request.continuation)
                except (ValueError, TypeError):
                    start_index = 0

            end_index = min(start_index + PAGE_SIZE,
                            len(partition_data[pkrange_id]))
            items = partition_data[pkrange_id][start_index:end_index]

            # Format items as Python objects (not JSON strings)
            if "ORDER BY" in scenario.query_sql.upper():
                # For ordered queries, use the orderByItems format
                formatted_items = [
                    {
                        "orderByItems": [{"item": item.value}],
                        "payload": item.to_dict()
                    }
                    for item in items
                ]
            else:
                # For unordered queries, use simple dict format
                formatted_items = [item.to_dict() for item in items]

            # Determine continuation token
            continuation = str(end_index) if end_index < len(
                partition_data[pkrange_id]) else None

            # Provide data to pipeline
            pipeline.provide_data(pkrange_id, formatted_items, continuation)


def run_benchmark_scenario(
    scenario: BenchmarkScenario,
    items_per_partition: int
) -> int:
    """Run a single benchmark scenario and return total items processed."""
    # Pre-create test data
    partition_data: Dict[str, List[BenchmarkItem]] = {}
    for i in range(PARTITION_COUNT):
        partition_id = f"partition_{i}"
        partition_data[partition_id] = create_partition_data(
            partition_id, items_per_partition)

    # Create query plan and partition ranges
    query_plan = scenario.query_plan_fn()
    partition_ranges = create_partition_key_ranges(PARTITION_COUNT)

    # Create pipeline
    engine = azure_cosmoscx.QueryEngine()
    pipeline = engine.create_pipeline(
        scenario.query_sql,
        query_plan,
        partition_ranges
    )

    total_items = 0

    # Run the pipeline until completion
    while True:
        result = pipeline.next_batch()

        # Count items yielded by this turn
        total_items += len(result.items)

        # If pipeline is terminated, we're done
        if result.terminated:
            break

        # Fulfill data requests
        fulfill_data_requests(
            pipeline,
            result.requests,
            partition_data,
            scenario
        )

    # Verify we processed all expected items
    expected_total = PARTITION_COUNT * items_per_partition
    assert total_items == expected_total, f"Expected {expected_total} items, got {total_items}"

    return total_items


class TestThroughputBenchmarks:
    """Throughput benchmark tests."""

    @pytest.mark.parametrize("items_per_partition", [100])
    def test_unordered_throughput(self, benchmark, items_per_partition):
        """Benchmark unordered query throughput."""
        scenario = BenchmarkScenario(
            "unordered",
            "SELECT * FROM c",
            create_simple_query_plan,
            unordered_item_formatter
        )

        # Calculate expected total items
        expected_total = PARTITION_COUNT * items_per_partition

        # Run the benchmark and measure items per second
        def benchmark_with_throughput():
            start_time = time.perf_counter()
            total_items = run_benchmark_scenario(scenario, items_per_partition)
            end_time = time.perf_counter()
            elapsed_time = end_time - start_time
            items_per_second = total_items / elapsed_time
            return total_items, items_per_second

        # Run the benchmark
        result = benchmark(benchmark_with_throughput)
        total_items, items_per_second = result

        # Print throughput information
        print(f"\n{scenario.name.capitalize()} Query Throughput:")
        print(f"  Total items processed: {total_items}")
        print(f"  Items per second: {items_per_second:,.0f}")

        # Verify the expected number of items were processed
        assert total_items == expected_total

    @pytest.mark.parametrize("items_per_partition", [100])
    def test_ordered_throughput(self, benchmark, items_per_partition):
        """Benchmark ordered query throughput."""
        scenario = BenchmarkScenario(
            "ordered",
            "SELECT * FROM c ORDER BY c.value",
            create_ordered_query_plan,
            ordered_item_formatter
        )

        # Calculate expected total items
        expected_total = PARTITION_COUNT * items_per_partition

        # Run the benchmark and measure items per second
        def benchmark_with_throughput():
            start_time = time.perf_counter()
            total_items = run_benchmark_scenario(scenario, items_per_partition)
            end_time = time.perf_counter()
            elapsed_time = end_time - start_time
            items_per_second = total_items / elapsed_time
            return total_items, items_per_second

        # Run the benchmark
        result = benchmark(benchmark_with_throughput)
        total_items, items_per_second = result

        # Print throughput information
        print(f"\n{scenario.name.capitalize()} Query Throughput:")
        print(f"  Total items processed: {total_items}")
        print(f"  Items per second: {items_per_second:,.0f}")

        # Verify the expected number of items were processed
        assert total_items == expected_total
