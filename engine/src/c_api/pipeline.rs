use crate::{
    c_api::{result::ResultExt, slice::OwnedSlice},
    query::{self, PartitionKeyRange},
    ErrorKind,
};

use super::{
    result::{FfiResult, ResultCode},
    slice::{OwnedString, Str},
};

// The C API uses "Box<serde_json::value::RawValue>" as the payload type for the query pipeline.
type RawQueryPipeline = query::QueryPipeline<Box<serde_json::value::RawValue>>;
type RawQueryResult = query::QueryResult<Box<serde_json::value::RawValue>>;

/// Opaque type representing the query pipeline.
/// Callers should not attempt to access the fields of this struct directly.
pub struct Pipeline;

/// Creates a new query pipeline from a JSON query plan and list of partitions.
///
/// # Parameters
/// - `query_plan_json`: A [`Str`] containing the serialized query plan, as recieved from the gateway, in JSON.
/// - `pkranges`: A [`Str`] containing the serialized partition key ranges list, as recieved from the gateway, in JSON.
#[no_mangle]
extern "C" fn cosmoscx_v0_query_pipeline_create<'a>(
    query_plan_json: Str<'a>,
    pkranges: Str<'a>,
) -> FfiResult<Pipeline> {
    fn inner<'a>(
        query_plan_json: Str<'a>,
        pkranges: Str<'a>,
    ) -> crate::Result<Box<RawQueryPipeline>> {
        let query_plan_json =
            unsafe { query_plan_json.as_str() }?.ok_or_else(|| ErrorKind::ArgumentNull)?;
        let query_plan: query::QueryPlan = serde_json::from_str(query_plan_json)
            .map_err(|e| crate::ErrorKind::InvalidGatewayResponse.with_source(e))?;

        let pkranges_json = unsafe { pkranges.as_str() }?.ok_or_else(|| ErrorKind::ArgumentNull)?;
        let pkranges: Vec<PartitionKeyRange> = serde_json::from_str(pkranges_json)
            .map_err(|e| crate::ErrorKind::InvalidGatewayResponse.with_source(e))?;

        // SAFETY: We should no longer need either of the parameter slices, we copied them into owned data.

        tracing::debug!(query_plan = ?query_plan, pkranges = ?pkranges, "created query pipeline");
        let pipeline = RawQueryPipeline::new(query_plan, pkranges)?;
        Ok(Box::new(pipeline))
    }

    inner(query_plan_json, pkranges).into()
}

/// Frees the memory associated with a pipeline.
#[no_mangle]
extern "C" fn cosmoscx_v0_query_pipeline_free(pipeline: *mut Pipeline) {
    unsafe { crate::c_api::free(pipeline) }
}

#[repr(C)]
pub struct DataRequest {
    /// The Partition Key Range ID to request data from.
    pkrangeid: OwnedString,

    /// The continuation token to provide, or an empty slice (len == 0) if no continuation should be provided.
    continuation: OwnedString,
}

#[repr(C)]
pub struct PipelineResult {
    /// A boolean indicating if the pipeline has completed.
    completed: bool,

    /// A [`Slice`] of [`Str`]s containing the JSON for each item in the output.
    items: OwnedSlice<OwnedString>,

    /// A [`Slice`] of [`DataRequest`]s describing additional requests that must be made and provided to [`cosmoscx_v0_query_pipeline_provide_data`] before retrieving the next batch.
    requests: OwnedSlice<DataRequest>,
}

/// Fetches the next batch of query results.
///
/// The [`PipelineResult`] returned here MUST be freed using [`cosmoscx_v0_query_pipeline_free_result`].
#[no_mangle]
extern "C" fn cosmoscx_v0_query_pipeline_next_batch<'a>(
    pipeline: *mut Pipeline,
) -> FfiResult<PipelineResult> {
    fn inner<'a>(pipeline: *mut Pipeline) -> crate::Result<Box<PipelineResult>> {
        let pipeline = unsafe {
            (pipeline as *mut RawQueryPipeline)
                .as_mut()
                .ok_or_else(|| ErrorKind::ArgumentNull.with_message("pipeline was null"))
        }?;
        let batch = pipeline.next_batch()?;

        let result = if let Some(batch) = batch {
            // Box up each of the JSON values in the batch.
            let items = batch
                .batch
                .into_iter()
                .map(|r| Box::<str>::from(r).into_boxed_bytes().into())
                .collect::<Vec<_>>()
                .into();

            // And box up the requests.
            let requests = batch
                .requests
                .into_iter()
                .map(|r| DataRequest {
                    pkrangeid: r.pkrange_id.into_owned().into(),
                    continuation: match r.continuation {
                        None => OwnedSlice::EMPTY,
                        Some(s) => s.into(),
                    },
                })
                .collect::<Vec<_>>()
                .into();
            Box::new(PipelineResult {
                completed: false,
                items,
                requests,
            })
        } else {
            Box::new(PipelineResult {
                completed: true,
                items: OwnedSlice::EMPTY,
                requests: OwnedSlice::EMPTY,
            })
        };

        Ok(result)
    }

    inner(pipeline).into()
}

/// Frees all the memory associated with a [`PipelineResult`].
///
/// Calling this function will release all the strings and buffers provided within the [`PipelineResult`], so ensure you have copied it all out before calling this.
#[no_mangle]
extern "C" fn cosmoscx_v0_query_pipeline_free_result<'a>(result: *mut PipelineResult) {
    unsafe { crate::c_api::free(result) }
}

/// Inserts additional raw data, in response to a [`DataRequest`] from the pipeline.
#[no_mangle]
extern "C" fn cosmoscx_v0_query_pipeline_provide_data<'a>(
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
    ) -> crate::Result<()> {
        let pipeline = unsafe {
            (pipeline as *mut RawQueryPipeline)
                .as_mut()
                .ok_or_else(|| ErrorKind::ArgumentNull.with_message("pipeline was null"))
        }?;

        // Parse the data
        let pkrange_id = unsafe { pkrange_id.as_str().not_null()? };

        // TODO: Only queries with order by/group by will come back from the server formatted as QueryResults. The rest will be raw payloads!
        // We need to handle that

        let query_results: Vec<RawQueryResult> =
            serde_json::from_str(unsafe { data.as_str().not_null()? })
                .map_err(|e| ErrorKind::DeserializationError.with_source(e))?;
        let continuation = unsafe {
            match continuation.into_string()? {
                // Normalize empty strings to 'None'
                Some(s) if s.is_empty() => None,
                x => x,
            }
        };

        // And insert it!
        pipeline.provide_data(pkrange_id, query_results, continuation)
    }

    inner(pipeline, pkrange_id, data, continuation).into()
}
