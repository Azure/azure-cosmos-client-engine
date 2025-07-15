# Benchmarks

This directory contains benchmarks for the Azure Cosmos DB Python client engine.

## Structure

- `test_throughput.py` - Throughput benchmarks that measure the raw performance of the query pipeline

## Running Benchmarks

To run the throughput benchmarks:

```bash
# Navigate to the Python directory
cd /workspaces/azure-cosmos-client-engine/python

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

## Benchmark Results

The benchmarks measure both the time taken to process all items through the query pipeline and the throughput in items per second. Key metrics include:

- **Mean time**: Average time per benchmark run
- **OPS (Operations Per Second)**: Number of benchmark runs per second (higher is better)
- **Items per second**: Number of items processed per second (higher is better) - comparable to Rust/Go benchmarks
- **Min/Max**: Fastest and slowest individual runs
- **StdDev**: Standard deviation showing consistency of results

### Example Results

- **Unordered Query**: ~325 microseconds mean time, ~3,073 OPS, ~1,309,000 items/second
- **Ordered Query**: ~1,224 microseconds mean time, ~817 OPS, ~335,000 items/second (about 3.9x slower due to sorting overhead)

The Python wrapper achieves approximately:

- **1.3 million items/second** for unordered queries
- **335,000 items/second** for ordered queries

## Benchmark Scenarios

The benchmarks are designed to be comparable with the Rust (`throughput.rs`) and Go (`benchmark_test.go`) benchmarks, measuring:

1. **Unordered Query Throughput** - Simple SELECT queries without ORDER BY
2. **Ordered Query Throughput** - SELECT queries with ORDER BY clauses

Each benchmark scenario simulates partitioned data and fulfills data requests dynamically to measure the end-to-end pipeline performance.
