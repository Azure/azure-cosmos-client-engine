/// A result code for FFI functions, which indicates the success or failure of the operation.
#[repr(isize)]
pub enum ResultCode {
    Success = 0,
    UnknownError = -1,
    QueryPlanInvalid = -2,
    DeserializationError = -3,
    UnknownPartitionKeyRange = -4,
    InternalError = -5,
    UnsupportedQueryPlan = -6,
    InvalidUtf8String = -7,
}

impl From<crate::Error> for ResultCode {
    fn from(value: crate::Error) -> Self {
        value.kind().into()
    }
}

impl From<crate::ErrorKind> for ResultCode {
    fn from(value: crate::ErrorKind) -> Self {
        match value {
            crate::ErrorKind::QueryPlanInvalid => ResultCode::QueryPlanInvalid,
            crate::ErrorKind::DeserializationError => ResultCode::DeserializationError,
            crate::ErrorKind::UnknownPartitionKeyRange => ResultCode::UnknownPartitionKeyRange,
            crate::ErrorKind::InternalError => ResultCode::InternalError,
            crate::ErrorKind::UnsupportedQueryPlan => ResultCode::UnsupportedQueryPlan,
            crate::ErrorKind::InvalidUtf8String => ResultCode::InvalidUtf8String,
        }
    }
}

/// A result type for FFI functions.
#[repr(C)]
pub struct FfiResult {
    code: ResultCode,
    value: *const std::ffi::c_void,
}

impl<T> From<Result<*const T, crate::Error>> for FfiResult {
    fn from(value: Result<*const T, crate::Error>) -> Self {
        // TODO: Store the error details in a thread local to be retrieved by a "get last error" function.

        match value {
            Ok(value) => Self {
                code: ResultCode::Success,
                value: value as *const T as *const std::ffi::c_void,
            },
            Err(e) => Self {
                code: e.into(),
                value: std::ptr::null(),
            },
        }
    }
}
