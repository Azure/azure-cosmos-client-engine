use serde::Deserialize;

use crate::{
    c_api::{result::ResultExt, slice::OwnedSlice},
    query::{JsonQueryClauseItem, PartitionKeyRange, QueryPipeline, QueryPlan},
    ErrorKind,
};

use super::{
    result::{FfiResult, ResultCode},
    slice::{OwnedString, Str},
};

// The C API uses "Box<serde_json::value::RawValue>" as the payload type for the query pipeline.
type RawQueryPipeline = QueryPipeline<Box<serde_json::value::RawValue>, JsonQueryClauseItem>;

/// Opaque type representing the query pipeline.
/// Callers should not attempt to access the fields of this struct directly.
pub struct Pipeline;

impl Pipeline {
    // We can't make this into a "method" without the arbitrary_self_types feature
    // (https://github.com/rust-lang/rust/issues/44874)
    pub unsafe fn unwrap_ptr(pipeline: *mut Self) -> crate::Result<&'static mut RawQueryPipeline> {
        (pipeline as *mut RawQueryPipeline)
            .as_mut()
            .ok_or_else(|| ErrorKind::ArgumentNull.with_message("pipeline was null"))
    }
}

/// Creates a new query pipeline from a JSON query plan and list of partitions.
///
/// # Parameters
/// - `query_plan_json`: A [`Str`] containing the serialized query plan, as recieved from the gateway, in JSON.
/// - `pkranges`: A [`Str`] containing the serialized partition key ranges list, as recieved from the gateway, in JSON.
#[no_mangle]
extern "C" fn cosmoscx_v0_query_pipeline_create<'a>(
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
    ) -> crate::Result<Box<RawQueryPipeline>> {
        let query = unsafe { query.as_str().not_null() }?;
        let query_plan_json = unsafe { query_plan_json.as_str().not_null() }?;
        let pkranges_json = unsafe { pkranges.as_str().not_null() }?;

        let query_plan: QueryPlan = serde_json::from_str(query_plan_json)
            .map_err(|e| crate::ErrorKind::InvalidGatewayResponse.with_source(e))?;
        let pkranges: PartitionKeyRangeResult = serde_json::from_str(pkranges_json)
            .map_err(|e| crate::ErrorKind::InvalidGatewayResponse.with_source(e))?;

        // SAFETY: We should no longer need either of the parameter slices, we copied them into owned data.

        tracing::debug!(query = ?query, query_plan = ?query_plan, pkranges = ?pkranges.ranges, "creating query pipeline");
        let pipeline = RawQueryPipeline::new(query, query_plan, pkranges.ranges)?;
        Ok(Box::new(pipeline))
    }

    inner(query, query_plan_json, pkranges).into()
}

/// Frees the memory associated with a pipeline.
#[no_mangle]
extern "C" fn cosmoscx_v0_query_pipeline_free(pipeline: *mut Pipeline) {
    unsafe { crate::c_api::free(pipeline) }
}

/// Gets the, possibly rewritten, query that this pipeline is executing.
///
/// The string returned here should be copied to a language-specific string type before being used.
/// It remains valid until the pipeline is freed by a call to [`cosmoscx_v0_query_pipeline_free`].
#[no_mangle]
extern "C" fn cosmoscx_v0_query_pipeline_query(pipeline: *mut Pipeline) -> FfiResult<Str<'static>> {
    fn inner(pipeline: *mut Pipeline) -> crate::Result<Box<Str<'static>>> {
        let pipeline = unsafe { Pipeline::unwrap_ptr(pipeline) }?;
        Ok(Box::new(pipeline.query().into()))
    }

    inner(pipeline).into()
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
        let pipeline = unsafe { Pipeline::unwrap_ptr(pipeline) }?;
        let batch = pipeline.next_batch()?;

        let result = if let Some(batch) = batch {
            // Box up each of the JSON values in the batch.
            let items = batch
                .items
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

        let query_results = pipeline.deserialize_payload(data)?;

        // And insert it!
        pipeline.provide_data(pkrange_id, query_results, continuation)
    }

    inner(pipeline, pkrange_id, data, continuation).into()
}
