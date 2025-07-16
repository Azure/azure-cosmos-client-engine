# Copyright (c) Microsoft Corporation.
# Licensed under the MIT License.

"""
Utility functions and classes for benchmarks.
"""

import json
from typing import Dict, List, Any
from .config import PARTITION_COUNT, PAGE_SIZE


class BenchmarkItem:
    """Simple test item for benchmarking."""

    def __init__(self, id: str, partition_key: str, value: int):
        self.id = id
        self.partition_key = partition_key
        self.value = value
        self.description = f"Item {id} in partition {partition_key}"

    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary representation."""
        return {
            "id": self.id,
            "partition_key": self.partition_key,
            "value": self.value,
            "description": self.description
        }


def create_partition_data(partition_id: str, item_count: int) -> List[BenchmarkItem]:
    """Create test data for a partition."""
    return [
        BenchmarkItem(f"item_{i}", partition_id, i)
        for i in range(item_count)
    ]


def create_partition_key_ranges(count: int) -> List[Dict[str, str]]:
    """Create partition key ranges for testing."""
    return [
        {
            "id": f"partition_{i}",
            "minInclusive": f"{i * 10:02X}",
            "maxExclusive": f"{(i + 1) * 10:02X}"
        }
        for i in range(count)
    ]


def unordered_item_formatter(item: BenchmarkItem) -> str:
    """Format item for unordered queries."""
    return json.dumps({
        "id": item.id,
        "partition_key": item.partition_key,
        "value": item.value,
        "description": item.description
    }, separators=(',', ':'))


def ordered_item_formatter(item: BenchmarkItem) -> str:
    """Format item for ordered queries."""
    return json.dumps({
        "payload": {
            "id": item.id,
            "partition_key": item.partition_key,
            "value": item.value,
            "description": item.description
        },
        "orderByItems": [{"item": item.value}]
    }, separators=(',', ':'))


def create_simple_query_plan() -> Dict[str, Any]:
    """Create a simple unordered query plan."""
    return {
        "partitionedQueryExecutionInfoVersion": 1,
        "queryInfo": {
            "distinctType": "None"
        },
        "queryRanges": []
    }


def create_ordered_query_plan() -> Dict[str, Any]:
    """Create a simple ordered query plan."""
    return {
        "partitionedQueryExecutionInfoVersion": 1,
        "queryInfo": {
            "distinctType": "None",
            "orderBy": ["Ascending"]
        },
        "queryRanges": []
    }
