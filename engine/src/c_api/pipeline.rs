use crate::query::{self, QueryPlan};

use super::result::{FfiResult, ResultCode};

// The C API uses "Box<serde_json::value::RawValue>" as the payload type for the query pipeline.
type RawQueryPipeline = query::QueryPipeline<Box<serde_json::value::RawValue>>;

/// Opaque type representing the query pipeline.
/// Callers should not attempt to access the fields of this struct directly.
pub struct Pipeline;

/// Represents a contiguous sequence of objects.
///
/// This struct is used to pass a sequence of objects from the caller to the engine.
/// The engine will never take ownership over the data in the slice, and will use it only for the duration of a single function call.
/// The documentation for each function or struct that references `Slice` will specify what type of data is expected in the slice.
///
/// This type shouldn't actually appear on the public API. Instead, it will be typedef'd to more specific slice types.
// TODO: We could use a zero-sized generic field here to enforce the type of the data on the Rust side. Unfortunately, cbindgen doesn't generate helpful code when we do that:
// https://github.com/mozilla/cbindgen/issues/1045
#[repr(C)]
pub struct Slice<'a> {
    data: *const std::ffi::c_void,
    len: usize,
    _phantom: std::marker::PhantomData<&'a ()>,
}

impl<'a> Slice<'a> {
    pub unsafe fn as_slice<T>(&self) -> &'a [T] {
        std::slice::from_raw_parts(self.data as *const T, self.len)
    }
}

/// Represents a string of UTF-8 characters.
///
/// This is effectively the same as a [`Slice`] of [`u8`], but it is used to clearly indicate that the data is a string.
#[repr(transparent)]
pub struct Str<'a>(Slice<'a>);

impl<'a> Str<'a> {
    pub unsafe fn as_str(&self) -> crate::Result<&'a str> {
        let slice = self.0.as_slice::<u8>();
        std::str::from_utf8(slice).map_err(|_| crate::ErrorKind::InvalidUtf8String.into())
    }
}

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
    partitions: Slice<'a>,
) -> FfiResult {
    fn inner<'a>(
        query_plan_json: Str<'a>,
        partitions: Slice<'a>,
    ) -> crate::Result<*const Pipeline> {
        let query_plan_json = unsafe { query_plan_json.as_str() }?;
        let query_plan: QueryPlan = serde_json::from_str(query_plan_json)
            .map_err(|e| crate::ErrorKind::QueryPlanInvalid.with_source(e))?;

        let partitions = unsafe { partitions.as_slice::<PartitionKeyRange>() }
            .iter()
            .map(|p| -> crate::Result<query::PartitionKeyRange> {
                let id = unsafe { p.id.as_str()? }.to_string();
                let min_inclusive = unsafe { p.min_inclusive.as_str()? }.to_string();
                let max_exclusive = unsafe { p.max_exclusive.as_str()? }.to_string();
                Ok(query::PartitionKeyRange::new(
                    id,
                    min_inclusive,
                    max_exclusive,
                ))
            })
            .collect::<Result<Vec<_>, _>>()?;

        // SAFETY: We should no longer need either of the parameter slices, we copied them into owned data.

        tracing::debug!(query_plan = ?query_plan, partitions = ?partitions, "creating query pipeline");
        let pipeline = RawQueryPipeline::new(query_plan, partitions)?;

        // Box and "leak" the pipeline to the caller.
        // The caller is now responsible for calling `cosmoscx_query_pipeline_free` to free the memory.
        Ok(Box::into_raw(Box::new(pipeline)) as *const _)
    }

    inner(query_plan_json, partitions).into()
}

#[no_mangle]
extern "C" fn cosmoscx_v0_query_pipeline_free(pipeline: *mut Pipeline) {
    // SAFETY: We have to trust that the caller is giving us a valid pipeline created by `cosmoscx_query_pipeline_create`.
    unsafe {
        // Return the pointer to a Box, and drop it.
        drop(Box::from_raw(pipeline as *mut RawQueryPipeline))
    };
    tracing::debug!("query pipeline freed");
}
