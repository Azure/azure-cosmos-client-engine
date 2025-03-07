/// A result code for FFI functions, which indicates the success or failure of the operation.
/// cbindgen:prefix-with-name
/// cbindgen:rename-all=SCREAMING_SNAKE_CASE
#[repr(isize)]
pub enum ResultCode {
    Success = 0,
    UnknownError = -1,
    InvalidGatewayResponse = -2,
    DeserializationError = -3,
    UnknownPartitionKeyRange = -4,
    InternalError = -5,
    UnsupportedQueryPlan = -6,
    InvalidUtf8String = -7,
    ArgumentNull = -8,
}

impl From<crate::Error> for ResultCode {
    fn from(value: crate::Error) -> Self {
        value.kind().into()
    }
}

impl From<crate::ErrorKind> for ResultCode {
    fn from(value: crate::ErrorKind) -> Self {
        match value {
            crate::ErrorKind::InvalidGatewayResponse => ResultCode::InvalidGatewayResponse,
            crate::ErrorKind::DeserializationError => ResultCode::DeserializationError,
            crate::ErrorKind::UnknownPartitionKeyRange => ResultCode::UnknownPartitionKeyRange,
            crate::ErrorKind::InternalError => ResultCode::InternalError,
            crate::ErrorKind::UnsupportedQueryPlan => ResultCode::UnsupportedQueryPlan,
            crate::ErrorKind::InvalidUtf8String => ResultCode::InvalidUtf8String,
            crate::ErrorKind::ArgumentNull => ResultCode::ArgumentNull,

            // This shouldn't happen, since we don't use ResultCode in the python module
            // The only reason this isn't cfg'd out is to allow us to do a simple --all-features build.
            #[cfg(feature = "python")]
            crate::ErrorKind::PythonError => ResultCode::InternalError,
        }
    }
}

impl From<Result<(), crate::Error>> for ResultCode {
    fn from(value: Result<(), crate::Error>) -> Self {
        match value {
            Ok(_) => ResultCode::Success,
            Err(e) => {
                tracing::error!(error = ?e, "an error occurred");
                // TODO: Store the error details in a thread local to be retrieved by a "get last error" function.

                e.into()
            }
        }
    }
}

/// A result type for FFI functions.
#[repr(C)]
pub struct FfiResult<T> {
    code: ResultCode,
    value: *const T,
}

impl<T, U> From<Result<Box<T>, crate::Error>> for FfiResult<U> {
    fn from(value: Result<Box<T>, crate::Error>) -> Self {
        match value {
            Ok(value) => {
                let ptr = Box::into_raw(value) as *const U;
                tracing::trace!(?ptr, typ = std::any::type_name::<Box<T>>(), "allocated");
                Self {
                    code: ResultCode::Success,
                    value: ptr,
                }
            }
            Err(e) => {
                tracing::error!(error = ?e, "an error occurred");
                // TODO: Store the error details in a thread local to be retrieved by a "get last error" function.

                Self {
                    code: e.into(),
                    value: std::ptr::null(),
                }
            }
        }
    }
}

pub trait ResultExt {
    type Output;
    fn not_null(self) -> Result<Self::Output, crate::Error>;
}

impl<T> ResultExt for Result<Option<T>, crate::Error> {
    type Output = T;
    fn not_null(self) -> Result<Self::Output, crate::Error> {
        match self {
            Ok(Some(t)) => Ok(t),
            Err(e) => Err(e),
            Ok(None) => Err(crate::ErrorKind::ArgumentNull.into()),
        }
    }
}
