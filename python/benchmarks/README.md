# Benchmarks

This directory contains benchmarks for the Azure Cosmos DB Python client engine.

## Structure

- `test_throughput.py` - Throughput benchmarks that measure the raw performance of the query pipeline

## Running Benchmarks

To run the throughput benchmarks:

```bash
# Navigate to the Python directory
cd python

# Install dependencies
poetry install

# Run all benchmarks
poetry run pytest benchmarks/test_throughput.py --benchmark-only

# Run with verbose output and save results
poetry run pytest benchmarks/test_throughput.py --benchmark-only --benchmark-verbose --benchmark-save=throughput

# Run only unordered throughput benchmark
poetry run pytest benchmarks/test_throughput.py::TestThroughputBenchmarks::test_unordered_throughput --benchmark-only

# Run only ordered throughput benchmark
poetry run pytest benchmarks/test_throughput.py::TestThroughputBenchmarks::test_ordered_throughput --benchmark-only
```
