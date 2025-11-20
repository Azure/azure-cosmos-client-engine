# Release History

## 0.4.0 (Unreleased)

### Features Added

* Added support for hierarchical partitition keys to get the effective partition key string. - [PR 54](https://github.com/Azure/azure-cosmos-client-engine/pull/54)

### Breaking Changes

### Bugs Fixed

### Other Change

## 0.3.0 (2025-11-20)

### Breaking Changes
* Support for Hybrid and Full-Text queries - [PR 51](https://github.com/Azure/azure-cosmos-client-engine/pull/51)
* Update APIs to support providing multiple results to the pipeline at once - [PR 51](https://github.com/Azure/azure-cosmos-client-engine/pull/51)

## 0.2.0 (2025-11-04)

### Features Added

* Optimized partition key range selection logic to filter out ranges not relevant to the query. - [PR 47](https://github.com/Azure/azure-cosmos-client-engine/pull/47)

## 0.1.0 (2025-10-22)

### Features Added

* Added ability to get effective partition key string from a partition key value. - [PR 39](https://github.com/Azure/azure-cosmos-client-engine/pull/39)
* Added support for SELECT VALUE queries that include aggregate functions. - [PR 43](https://github.com/Azure/azure-cosmos-client-engine/pull/43)

## 0.0.5 (2025-02-25)

### Features Added

* Initial preview release of the Go wrapper.
