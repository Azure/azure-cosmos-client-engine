// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

//! Provides an implementation of the Azure Data Cosmos SDK query engine API.

use core::str;

use serde::Deserialize;

use crate::query::{JsonQueryClauseItem, PartitionKeyRange, QueryPipeline};

pub struct QueryEngine;

impl azure_data_cosmos::query::QueryEngine for QueryEngine {
    fn create_pipeline(
        &self,
        query: &str,
        plan: &[u8],
        pkranges: &[u8],
    ) -> azure_core::Result<Box<dyn azure_data_cosmos::query::QueryPipeline + Send>> {
        #[derive(Deserialize)]
        struct PartitionKeyRangeResult {
            #[serde(rename = "PartitionKeyRanges")]
            pub ranges: Vec<PartitionKeyRange>,
        }

        let plan = serde_json::from_slice(plan)?;
        let pkranges: PartitionKeyRangeResult = serde_json::from_slice(pkranges)?;
        let pipeline = QueryPipeline::<Box<serde_json::value::RawValue>, JsonQueryClauseItem>::new(
            query,
            plan,
            pkranges.ranges,
        )?;

        Ok(Box::new(QueryPipelineAdapter(pipeline)))
    }

    fn supported_features(&self) -> azure_core::Result<&str> {
        Ok(crate::query::SUPPORTED_FEATURES.as_str())
    }
}

impl From<crate::Error> for azure_core::Error {
    fn from(err: crate::Error) -> Self {
        let kind = match err.kind() {
            crate::ErrorKind::DeserializationError => azure_core::error::ErrorKind::DataConversion,
            crate::ErrorKind::UnknownPartitionKeyRange => {
                azure_core::error::ErrorKind::DataConversion
            }
            crate::ErrorKind::UnsupportedQueryPlan => azure_core::error::ErrorKind::DataConversion,
            crate::ErrorKind::InvalidUtf8String => azure_core::error::ErrorKind::DataConversion,
            _ => azure_core::error::ErrorKind::Other,
        };
        let message = format!("{}", &err);
        azure_core::Error::full(kind, err, message)
    }
}

pub struct QueryPipelineAdapter(
    crate::query::QueryPipeline<Box<serde_json::value::RawValue>, JsonQueryClauseItem>,
);

impl azure_data_cosmos::query::QueryPipeline for QueryPipelineAdapter {
    fn query(&self) -> &str {
        self.0.query()
    }

    fn complete(&self) -> bool {
        self.0.complete()
    }

    fn run(&mut self) -> azure_core::Result<azure_data_cosmos::query::PipelineResult> {
        let result = self.0.run()?;
        Ok(azure_data_cosmos::query::PipelineResult {
            is_completed: result.terminated,
            items: result.items,
            requests: result
                .requests
                .into_iter()
                .map(|request| azure_data_cosmos::query::QueryRequest {
                    partition_key_range_id: request.pkrange_id.into_owned(),
                    continuation: request.continuation,
                })
                .collect(),
        })
    }

    fn provide_data(
        &mut self,
        data: azure_data_cosmos::query::QueryResult,
    ) -> azure_core::Result<()> {
        let result = str::from_utf8(data.result)
            .map_err(|_| azure_core::error::ErrorKind::DataConversion)?;
        tracing::debug!(payload = result, "providing data to pipeline");
        let payload = self.0.deserialize_payload(result)?;
        self.0
            .provide_data(data.partition_key_range_id, payload, data.next_continuation)?;
        Ok(())
    }
}
