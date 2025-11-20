// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

//! FFI-safe types for communicating errors and the result of fallible functions.

use azure_data_cosmos_engine::ErrorKind;

/// A result code for FFI functions, which indicates the success or failure of the operation.
///
/// Values of `ResultCode` have the same representation as the C type `intptr_t`
/// cbindgen:prefix-with-name
/// cbindgen:rename-all=SCREAMING_SNAKE_CASE
#[repr(isize)]
pub enum ResultCode {
    /// The operation was successful.
    Success = 0,

    /// See [`ErrorKind::InvalidGatewayResponse`].
    InvalidGatewayResponse = -2,

    /// See [`ErrorKind::DeserializationError`].
    DeserializationError = -3,

    /// See [`ErrorKind::UnknownPartitionKeyRange`].
    UnknownPartitionKeyRange = -4,

    /// See [`ErrorKind::InternalError`].
    InternalError = -5,

    /// See [`ErrorKind::UnsupportedQueryPlan`].
    UnsupportedQueryPlan = -6,

    /// See [`ErrorKind::InvalidUtf8String`].
    InvalidUtf8String = -7,

    /// See [`ErrorKind::ArgumentNull`].
    ArgumentNull = -8,

    /// See [`ErrorKind::ArithmeticOverflow`].
    ArithmeticOverflow = -9,

<<<<<<< HEAD
    /// See [`ErrorKind::IllegalArgumentError`].
    IllegalArgumentError = -10,
=======
    /// See [`ErrorKind::InvalidRequestId`].
    InvalidRequestId = -10,

    /// See [`ErrorKind::InvalidQuery`].
    InvalidQuery = -11,
>>>>>>> c2e0c297026a14a34b3756ddd350b70fa0f8beea
}

impl From<azure_data_cosmos_engine::Error> for ResultCode {
    /// Converts an [`azure_data_cosmos_engine::Error`] into a [`ResultCode`] by converting it's [`ErrorKind`].
    fn from(value: azure_data_cosmos_engine::Error) -> Self {
        value.kind().into()
    }
}

impl From<ErrorKind> for ResultCode {
    /// Converts an [`ErrorKind`] into a [`ResultCode`].
    fn from(value: ErrorKind) -> Self {
        match value {
            ErrorKind::InvalidGatewayResponse => ResultCode::InvalidGatewayResponse,
            ErrorKind::DeserializationError => ResultCode::DeserializationError,
            ErrorKind::UnknownPartitionKeyRange => ResultCode::UnknownPartitionKeyRange,
            ErrorKind::InternalError => ResultCode::InternalError,
            ErrorKind::UnsupportedQueryPlan => ResultCode::UnsupportedQueryPlan,
            ErrorKind::InvalidUtf8String => ResultCode::InvalidUtf8String,
            ErrorKind::ArgumentNull => ResultCode::ArgumentNull,
            ErrorKind::ArithmeticOverflow => ResultCode::ArithmeticOverflow,
            ErrorKind::InvalidRequestId => ResultCode::InvalidRequestId,
            ErrorKind::InvalidQuery => ResultCode::InvalidQuery,
            ErrorKind::PythonError => ResultCode::InternalError,
            ErrorKind::IllegalArgumentError => ResultCode::IllegalArgumentError,
        }
    }
}

impl From<Result<(), azure_data_cosmos_engine::Error>> for ResultCode {
    /// Converts the result of a fallible operation that returns no value (i.e. `Result<(), Error>`) into a [`ResultCode`].
    ///
    /// If the value is `Ok(())`, this returns [`ResultCode::Success`],
    /// otherwise it returns a [`ResultCode`] matching the [`ErrorKind`] of the [`Error`](azure_data_cosmos_engine::Error) that was raised.
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
///
/// An `FfiResult` is returned from a function that both returns a value AND can fail.
///
/// The C representation of this struct is:
///
/// ```
/// struct {
///   intptr_t code; // The result code, which will be '0' if the operation succeeded
///   const void *value; // A pointer to the returned value, which will be `nullptr`/`0` if the operation failed.
/// };
/// ```
///
/// The data pointed to by the `value` pointer is OWNED BY THE ENGINE and must be freed by calling the appropriate free function, depending on the data.
#[repr(C)]
pub struct FfiResult<T> {
    code: ResultCode,
    value: *const T,
}

impl<T, U> From<Result<Box<T>, azure_data_cosmos_engine::Error>> for FfiResult<U> {
    /// Consumes the result of a fallible function that returns a boxed value (i.e. `Result<Box<T>, Error>`) and returns an [`FfiResult`].
    ///
    /// If the provided result is `Err`, the [`FfiResult`] returned will have a non-zero `code` value and a null `value` pointer.
    /// If the provdied result is `Ok`, the [`FfiResult`] returned will have a zero `code` value and a `value` pointer pointing to the same memory as the `Box<T>` provided.
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

/// Extension trait that adds the [`ResultExt::not_null`] method to `Result<Option<T>, Error>`.
pub trait ResultExt {
    /// The type of the `Ok` value of the result.
    type Output;

    /// Converts a `Result<Option<T>, Error>` into a `Result<T, Error>` where `None` values
    /// are replaced by `Err` values that yield the error [`ErrorKind::ArgumentNull`].
    ///
    /// This allows the caller to "assume" the provided value is non-null and produce an appropriate error if it is not.
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
