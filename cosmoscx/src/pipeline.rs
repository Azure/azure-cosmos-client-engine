// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

//! Functions related to creating and executing query pipelines.

use azure_data_cosmos_engine::{
    query::{ItemIdentity, PartitionKeyRange, QueryPipeline, QueryPlan, ReadManyPipeline},
    ErrorKind,
};
use serde::Deserialize;

use crate::{result::ResultExt, slice::OwnedSlice};

use super::{
    result::{FfiResult, ResultCode},
    slice::{OwnedString, Str},
};

/// Opaque type representing the query pipeline.
/// Callers should not attempt to access the fields of this struct directly.
pub struct Pipeline;

impl Pipeline {
    // We can't make this into a "method" without the arbitrary_self_types feature
    // (https://github.com/rust-lang/rust/issues/44874)

    /// Unwraps the pointer to the underlying `QueryPipeline` type.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the pointer passed to this function is a valid pointer to a `QueryPipeline`.
    pub unsafe fn unwrap_ptr(
        pipeline: *mut Self,
    ) -> Result<&'static mut QueryPipeline, azure_data_cosmos_engine::Error> {
        (pipeline as *mut QueryPipeline)
            .as_mut()
            .ok_or_else(|| ErrorKind::ArgumentNull.with_message("pipeline was null"))
    }
}

/// Creates a new query pipeline from a JSON query plan and list of partitions.
///
/// # Parameters
/// - `query`: A [`Str`] containing the query to be executed.
/// - `query_plan_json`: A [`Str`] containing the serialized query plan, as recieved from the gateway, in JSON.
/// - `pkranges`: A [`Str`] containing the serialized partition key ranges list, as recieved from the gateway, in JSON.
#[no_mangle]
pub extern "C" fn cosmoscx_v0_query_pipeline_create<'a>(
    query: Str<'a>,
    query_plan_json: Str<'a>,
    pkranges: Str<'a>,
) -> FfiResult<Pipeline> {
    #[derive(Deserialize)]
    struct PartitionKeyRangeResult {
        #[serde(rename = "PartitionKeyRanges")]
        pub ranges: Vec<PartitionKeyRange>,
    }

    fn inner<'a>(
        query: Str<'a>,
        query_plan_json: Str<'a>,
        pkranges: Str<'a>,
    ) -> Result<Box<QueryPipeline>, azure_data_cosmos_engine::Error> {
        let query = unsafe { query.as_str().not_null() }?;
        let query_plan_json = unsafe { query_plan_json.as_str().not_null() }?;
        let pkranges_json = unsafe { pkranges.as_str().not_null() }?;

        let query_plan: QueryPlan = serde_json::from_str(query_plan_json)
            .map_err(|e| ErrorKind::InvalidGatewayResponse.with_source(e))?;
        let pkranges: PartitionKeyRangeResult = serde_json::from_str(pkranges_json)
            .map_err(|e| ErrorKind::InvalidGatewayResponse.with_source(e))?;

        // SAFETY: We should no longer need either of the parameter slices, we copied them into owned data.

        tracing::debug!(query = ?query, query_plan = ?query_plan, pkranges = ?pkranges.ranges, "creating query pipeline");
        let pipeline = QueryPipeline::new(query, query_plan, pkranges.ranges)?;
        Ok(Box::new(pipeline))
    }

    inner(query, query_plan_json, pkranges).into()
}

/// Creates the relevant partition-scoped queries for executing the read many operation along with the pipeline to run them.
///
/// # Parameters
/// - `item_identities`: A [`Str`] containing the serialized item identities in JSON.
/// - `pkranges`: A [`Str`] containing the serialized partition key ranges list, as received from the gateway, in JSON.
/// - `pk_kind`: A [`Str`] containing the partition key kind.
/// - `pk_version`: The partition key version.
#[no_mangle]
pub extern "C" fn cosmoscx_v0_readmany_pipeline_create<'a>(
    item_identities: Str<'a>,
    pkranges: Str<'a>,
    pk_kind: Str<'a>,
    pk_version: u32,
) -> FfiResult<Pipeline> {
    #[derive(Deserialize)]
    struct PartitionKeyRangeResult {
        #[serde(rename = "PartitionKeyRanges")]
        pub ranges: Vec<PartitionKeyRange>,
    }
    #[derive(Deserialize, Debug)]
    struct ItemIdentitiesResult {
        #[serde(rename = "ItemIdentities")]
        pub identities: Vec<ItemIdentity>,
    }

    fn inner<'a>(
        item_identities: Str<'a>,
        pkranges: Str<'a>,
        pk_kind: Str<'a>,
        pk_version: u32,
    ) -> Result<Box<ReadManyPipeline>, azure_data_cosmos_engine::Error> {
        let item_identities_json = unsafe { item_identities.as_str().not_null() }?;
        let pkranges_json = unsafe { pkranges.as_str().not_null() }?;
        let pk_kind_json = unsafe { pk_kind.as_str().not_null() }?;
        let pk_version = pk_version;
        tracing::debug!(item_identities = ?item_identities_json, pkranges = ?pkranges_json, pk_kind = ?pk_kind_json, pk_version = ?pk_version, "parsing readmany pipeline parameters");

        let pkranges: PartitionKeyRangeResult = serde_json::from_str(pkranges_json)
            .map_err(|e| ErrorKind::InvalidGatewayResponse.with_source(e))?;
        let item_identities: ItemIdentitiesResult = serde_json::from_str(item_identities_json)
            .map_err(|e| ErrorKind::InvalidGatewayResponse.with_source(e))?;

        // SAFETY: We should no longer need either of the parameter slices, we copied them into owned data.

        tracing::debug!(item_identities = ?item_identities, pkranges = ?pkranges.ranges, pk_kind = ?pk_kind_json, pk_version = ?pk_version, "creating readmany pipeline");
        let pipeline = ReadManyPipeline::new(
            item_identities.identities,
            pkranges.ranges,
            pk_kind_json,
            pk_version
        )?;
        Ok(Box::new(pipeline))
    }

    inner(item_identities, pkranges, pk_kind, pk_version).into()
}

/// Frees the memory associated with a pipeline.
///
/// After calling this function, the memory pointed to by the `pointer` parameter becomes invalid.
///
/// # Safety
///
/// The caller must ensure that the pointer passed to this function is a valid pointer to a [`PipelineResult`] returned by [`cosmoscx_v0_query_pipeline_run`].
#[no_mangle]
pub unsafe extern "C" fn cosmoscx_v0_query_pipeline_free(pipeline: *mut Pipeline) {
    unsafe { crate::free(pipeline) }
}

/// Gets the, possibly rewritten, query that this pipeline is executing.
///
/// The string returned here should be copied to a language-specific string type before being used.
/// It remains valid until the pipeline is freed by a call to [`cosmoscx_v0_query_pipeline_free`].
#[no_mangle]
pub extern "C" fn cosmoscx_v0_query_pipeline_query(
    pipeline: *mut Pipeline,
) -> FfiResult<Str<'static>> {
    fn inner(
        pipeline: *mut Pipeline,
    ) -> Result<Box<Str<'static>>, azure_data_cosmos_engine::Error> {
        let pipeline = unsafe { Pipeline::unwrap_ptr(pipeline) }?;
        Ok(Box::new(pipeline.query().into()))
    }

    inner(pipeline).into()
}

/// Represents a request for more data from the pipeline.
///
/// Each `DataRequest` represents a request FROM the query pipeline to the calling SDK to perform a query against a single Cosmos partition.
#[repr(C)]
pub struct DataRequest {
    /// An [`OwnedString`] containing the Partition Key Range ID to request data from.
    pkrangeid: OwnedString,

    /// An [`OwnedString`] containing the continuation token to provide, or an empty slice (len == 0) if no continuation should be provided.
    continuation: OwnedString,

    /// An [`OwnedString`] containing the query to be executed.
    query: OwnedString,
}

/// Represents the result of a single execution of the query pipeline.
#[repr(C)]
pub struct PipelineResult {
    /// A boolean indicating if the pipeline has completed.
    completed: bool,

    /// An [`OwnedSlice`] of [`OwnedString`]s containing the JSON for each item in the output.
    items: OwnedSlice<OwnedString>,

    /// An [`OwnedSlice`] of [`DataRequest`]s describing additional requests that must be made and provided to [`cosmoscx_v0_query_pipeline_provide_data`] before retrieving the next batch.
    requests: OwnedSlice<DataRequest>,
}

/// Executes a single turn of the query pipeline.
///
/// See [`QueryPipeline::run`](azure_data_cosmos_engine::query::QueryPipeline::run) for more information on "turns".
///
/// The [`PipelineResult`] returned by this function MUST be freed using [`cosmoscx_v0_query_pipeline_free_result`] to release the memory associated with the result.
/// However, it does NOT need to be freed before the next time you call `cosmoscx_v0_query_pipeline_run`
/// You may have multiple outstanding un-freed [`PipelineResult`]s at once.
#[no_mangle]
pub extern "C" fn cosmoscx_v0_query_pipeline_run(
    pipeline: *mut Pipeline,
) -> FfiResult<PipelineResult> {
    fn inner(
        pipeline: *mut Pipeline,
    ) -> Result<Box<PipelineResult>, azure_data_cosmos_engine::Error> {
        let pipeline = unsafe { Pipeline::unwrap_ptr(pipeline) }?;
        let result = pipeline.run()?;

        // Box up each of the JSON values in the batch.
        let items = result
            .items
            .into_iter()
            .map(|r| Box::<str>::from(r).into_boxed_bytes().into())
            .collect::<Vec<_>>()
            .into();

        // And box up the requests.
        let requests = result
            .requests
            .into_iter()
            .map(|r| DataRequest {
                pkrangeid: r.pkrange_id.into_owned().into(),
                continuation: match r.continuation {
                    None => OwnedSlice::EMPTY,
                    Some(s) => s.into(),
                },
                query: match r.query {
                    None => OwnedSlice::EMPTY,
                    Some(s) => s.into(),
                },
            })
            .collect::<Vec<_>>()
            .into();

        Ok(Box::new(PipelineResult {
            completed: result.terminated,
            items,
            requests,
        }))
    }

    inner(pipeline).into()
}

/// Frees all the memory associated with a [`PipelineResult`].
///
/// Calling this function will release all the strings and buffers provided within the [`PipelineResult`], so ensure you have copied it all out before calling this.
///
/// # Safety
///
/// The caller must ensure that the pointer passed to this function is a valid pointer to a [`PipelineResult`] returned by [`cosmoscx_v0_query_pipeline_run`].
#[no_mangle]
pub unsafe extern "C" fn cosmoscx_v0_query_pipeline_free_result(result: *mut PipelineResult) {
    unsafe { crate::free(result) }
}

/// Inserts additional raw data, in response to a [`DataRequest`] from the pipeline.
#[no_mangle]
pub extern "C" fn cosmoscx_v0_query_pipeline_provide_data<'a>(
    pipeline: *mut Pipeline,
    pkrange_id: Str<'a>,
    data: Str<'a>,
    continuation: Str<'a>,
) -> ResultCode {
    fn inner<'a>(
        pipeline: *mut Pipeline,
        pkrange_id: Str<'a>,
        data: Str<'a>,
        continuation: Str<'a>,
    ) -> Result<(), azure_data_cosmos_engine::Error> {
        let pipeline = unsafe { Pipeline::unwrap_ptr(pipeline) }?;

        // Parse the data
        let pkrange_id = unsafe { pkrange_id.as_str().not_null()? };
        let data = unsafe { data.as_str().not_null()? };
        let continuation = unsafe {
            match continuation.into_string()? {
                // Normalize empty strings to 'None'
                Some(s) if s.is_empty() => None,
                x => x,
            }
        };

        // Pass the raw bytes directly to the pipeline
        pipeline.provide_data(pkrange_id, data.as_bytes(), continuation)
    }

    inner(pipeline, pkrange_id, data, continuation).into()
}
