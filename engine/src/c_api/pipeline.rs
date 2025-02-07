use crate::{
    c_api::{
        result::ResultExt,
        slice::{OwnedSlice, Slice},
    },
    query::{self, QueryPlan},
    ErrorKind,
};

use super::{
    result::FfiResult,
    slice::{OwnedString, Str},
};

// The C API uses "Box<serde_json::value::RawValue>" as the payload type for the query pipeline.
type RawQueryPipeline = query::QueryPipeline<Box<serde_json::value::RawValue>>;

/// Opaque type representing the query pipeline.
/// Callers should not attempt to access the fields of this struct directly.
pub struct Pipeline;

/// Describes a partition key range used to create a query pipeline.
/// cbindgen:export
#[repr(C)]
pub struct PartitionKeyRange<'a> {
    /// The ID of the partition key range.
    id: Str<'a>,

    /// The minimum value of the partition key range (inclusive).
    min_inclusive: Str<'a>,

    /// The maximum value of the partition key range (exclusive).
    max_exclusive: Str<'a>,
}

/// Creates a new query pipeline from a JSON query plan and list of partitions.
///
/// # Parameters
/// - `query_plan_json`: A [`Str`] containing the query plan as recieved from the gateway, in JSON.
/// - `partitions`: A [`Slice`] of [`PartitionKeyRange`] objects representing the partition key ranges to query.
#[no_mangle]
extern "C" fn cosmoscx_v0_query_pipeline_create<'a>(
    query_plan_json: Str<'a>,
    partitions: Slice<'a, PartitionKeyRange>,
) -> FfiResult<Pipeline> {
    fn inner<'a>(
        query_plan_json: Str<'a>,
        partitions: Slice<'a, PartitionKeyRange>,
    ) -> crate::Result<*const Pipeline> {
        let query_plan_json =
            unsafe { query_plan_json.as_str() }?.ok_or_else(|| ErrorKind::ArgumentNull)?;
        let query_plan: QueryPlan = serde_json::from_str(query_plan_json)
            .map_err(|e| crate::ErrorKind::QueryPlanInvalid.with_source(e))?;

        let partitions = unsafe { partitions.as_slice() }
            .ok_or_else(|| ErrorKind::ArgumentNull)?
            .iter()
            .map(|p| -> crate::Result<query::PartitionKeyRange> {
                let id = unsafe { p.id.as_str().not_null()? }.to_string();
                let min_inclusive = unsafe { p.min_inclusive.as_str().not_null()? }.to_string();
                let max_exclusive = unsafe { p.max_exclusive.as_str().not_null()? }.to_string();
                Ok(query::PartitionKeyRange::new(
                    id,
                    min_inclusive,
                    max_exclusive,
                ))
            })
            .collect::<Result<Vec<_>, _>>()?;

        // SAFETY: We should no longer need either of the parameter slices, we copied them into owned data.

        tracing::debug!(query_plan = ?query_plan, partitions = ?partitions, "created query pipeline");
        let pipeline = RawQueryPipeline::new(query_plan, partitions)?;

        let ptr = Box::into_raw(Box::new(pipeline)) as *const _;
        tracing::trace!(
            ?ptr,
            typ = std::any::type_name::<Box<RawQueryPipeline>>(),
            "allocating"
        );

        // Box and "leak" the pipeline to the caller.
        // The caller is now responsible for calling `cosmoscx_query_pipeline_free` to free the memory.
        Ok(ptr)
    }

    inner(query_plan_json, partitions).into()
}

/// Frees the memory associated with a pipeline.
#[no_mangle]
extern "C" fn cosmoscx_v0_query_pipeline_free(pipeline: *mut Pipeline) {
    // SAFETY: We have to trust that the caller is giving us a valid pipeline created by `cosmoscx_query_pipeline_create`.
    let owned = unsafe { Box::from_raw(pipeline as *mut RawQueryPipeline) };
    tracing::trace!(ptr = ?pipeline, typ = std::any::type_name_of_val(&owned), "free");
    drop(owned)
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
    fn inner<'a>(pipeline: *mut Pipeline) -> crate::Result<*const PipelineResult> {
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

        let ptr = Box::into_raw(result);
        tracing::trace!(
            ?ptr,
            typ = std::any::type_name::<Box<PipelineResult>>(),
            "allocating"
        );
        Ok(ptr)
    }

    inner(pipeline).into()
}

/// Frees all the memory associated with a [`PipelineResult`].
///
/// Calling this function will release all the strings and buffers provided within the [`PipelineResult`], so ensure you have copied it all out before calling this.
#[no_mangle]
extern "C" fn cosmoscx_v0_query_pipeline_free_result<'a>(result: *mut PipelineResult) {
    // SAFETY: We have to trust that the caller is giving us a valid pipeline result from calling "next_batch"
    let owned = unsafe { Box::from_raw(result) };
    tracing::trace!(ptr = ?result, typ = std::any::type_name_of_val(&owned), "freeing");
    drop(owned);
}
