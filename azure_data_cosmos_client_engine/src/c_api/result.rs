/// Defines the result codes for this library, indicating the success or failure of an operation
///
/// Zero, and positive values indicate success, while negative values indicate failure.
#[repr(isize)]
/// cbindgen:rename-all=SCREAMING_SNAKE_CASE
/// cbindgen:prefix-with-name
pub enum ResultCode {
    /// Indicates that the operation was successful
    Success = 0,

    /// Indicates that the operation failed for an unknown reason
    UnknownFailure = -1,
}

impl<E> From<Result<(), E>> for ResultCode
where
    ResultCode: From<E>,
{
    fn from(value: Result<(), E>) -> Self {
        match value {
            Ok(_) => ResultCode::Success,
            Err(e) => ResultCode::from(e),
        }
    }
}

#[repr(C)]
pub struct FfiResultPtr {
    pub result: ResultCode,
    pub ptr: *mut std::ffi::c_void,
}

impl<T> From<Box<T>> for FfiResultPtr {
    fn from(value: Box<T>) -> Self {
        FfiResultPtr {
            result: ResultCode::Success,
            ptr: Box::into_raw(value) as *mut std::ffi::c_void,
        }
    }
}

impl<T, E> From<Result<Box<T>, E>> for FfiResultPtr
where
    ResultCode: From<E>,
{
    fn from(value: Result<Box<T>, E>) -> Self {
        match value {
            Ok(t) => FfiResultPtr::from(t),
            Err(e) => FfiResultPtr {
                result: ResultCode::from(e),
                ptr: std::ptr::null_mut(),
            },
        }
    }
}

impl<T, E> From<Result<&T, E>> for FfiResultPtr
where
    ResultCode: From<E>,
{
    fn from(value: Result<&T, E>) -> Self {
        match value {
            Ok(t) => FfiResultPtr {
                result: ResultCode::Success,
                ptr: t as *const T as *mut std::ffi::c_void,
            },
            Err(e) => FfiResultPtr {
                result: ResultCode::from(e),
                ptr: std::ptr::null_mut(),
            },
        }
    }
}
