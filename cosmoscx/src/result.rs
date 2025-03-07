use azure_data_cosmos_engine::ErrorKind;

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

impl From<azure_data_cosmos_engine::Error> for ResultCode {
    fn from(value: azure_data_cosmos_engine::Error) -> Self {
        value.kind().into()
    }
}

impl From<ErrorKind> for ResultCode {
    fn from(value: ErrorKind) -> Self {
        match value {
            ErrorKind::InvalidGatewayResponse => ResultCode::InvalidGatewayResponse,
            ErrorKind::DeserializationError => ResultCode::DeserializationError,
            ErrorKind::UnknownPartitionKeyRange => ResultCode::UnknownPartitionKeyRange,
            ErrorKind::InternalError => ResultCode::InternalError,
            ErrorKind::UnsupportedQueryPlan => ResultCode::UnsupportedQueryPlan,
            ErrorKind::InvalidUtf8String => ResultCode::InvalidUtf8String,
            ErrorKind::ArgumentNull => ResultCode::ArgumentNull,
            ErrorKind::PythonError => ResultCode::InternalError,
        }
    }
}

impl From<Result<(), azure_data_cosmos_engine::Error>> for ResultCode {
    fn from(value: Result<(), azure_data_cosmos_engine::Error>) -> Self {
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

impl<T, U> From<Result<Box<T>, azure_data_cosmos_engine::Error>> for FfiResult<U> {
    fn from(value: Result<Box<T>, azure_data_cosmos_engine::Error>) -> Self {
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
    fn not_null(self) -> Result<Self::Output, azure_data_cosmos_engine::Error>;
}

impl<T> ResultExt for Result<Option<T>, azure_data_cosmos_engine::Error> {
    type Output = T;
    fn not_null(self) -> Result<Self::Output, azure_data_cosmos_engine::Error> {
        match self {
            Ok(Some(t)) => Ok(t),
            Err(e) => Err(e),
            Ok(None) => Err(ErrorKind::ArgumentNull.into()),
        }
    }
}
