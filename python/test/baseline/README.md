# Baseline Query Pipeline Tests

This directory contains tests that can run the Python built-in query engine ALONGSIDE the Rust query engine to test the correctness of the Rust engine.
These tests only run when `COSMOSCX_BASELINE_TEST` is set to `true`.

In order to run these tests, you need to have configured the following environment variables (though they all have defaults, so you can run the tests without setting them):

* `COSMOSCX_ENDPOINT`: The Cosmos endpoint to test against. Defaults to the local emulator.
* `COSMOSCX_KEY`: The key to use to authenticate. Defaults to the well-known emulator key.
* `COSMOSCX_DATABASE`: The database to test against. Defaults to `TestDB`.
* `COSMOSCX_CONTAINER`: The container to test against. Defaults to `TestContainer`.

The container you specify must have the sample data loaded into it.
You can do this by running the `script/load-sample-data` script with `testdata/sqlSampleData.json` as the sample data file.

Each test runs the same query against both engines and compares the results.