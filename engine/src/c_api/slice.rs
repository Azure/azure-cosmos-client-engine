use std::{marker::PhantomData, mem};

use crate::ErrorKind;

/// Represents a contiguous sequence of objects OWNED BY THE CALLING CODE.
///
/// The language binding owns this memory. It must keep the memory valid for the duration of any function call that receives it.
/// For example, the [`Slice`]s passed to [`cosmoscx_v0_query_pipeline_create`] must remain valid until that function returns.
/// After the function returns, the language binding may free the memory.
#[repr(C)]
pub struct Slice<'a, T> {
    data: *const T,
    len: usize,
    _phantom: PhantomData<&'a T>,
}

impl<'a, T> Slice<'a, T> {
    pub const EMPTY: Slice<'a, T> = Slice {
        data: std::ptr::null(),
        len: 0,
        _phantom: PhantomData,
    };

    pub unsafe fn as_slice(&self) -> Option<&'a [T]> {
        if self.data.is_null() {
            None
        } else {
            Some(std::slice::from_raw_parts(self.data as *const T, self.len))
        }
    }
}

impl<'a, T> From<&'a [T]> for Slice<'a, T> {
    fn from(value: &'a [T]) -> Self {
        Self {
            data: value.as_ptr(),
            len: value.len(),
            _phantom: PhantomData,
        }
    }
}

pub type Str<'a> = Slice<'a, u8>;

impl<'a> Str<'a> {
    pub unsafe fn as_str(&self) -> crate::Result<Option<&'a str>> {
        let Some(slice) = self.as_slice() else {
            return Ok(None);
        };
        Some(std::str::from_utf8(slice).map_err(|_| crate::ErrorKind::InvalidUtf8String.into()))
            .transpose()
    }

    pub unsafe fn into_string(&self) -> crate::Result<Option<String>> {
        self.as_str().map(|o| o.map(|s| s.to_string()))
    }
}

impl<'a> From<&'a str> for Str<'a> {
    fn from(value: &'a str) -> Self {
        value.as_bytes().into()
    }
}

/// Represents a contiguous sequence of objects OWNED BY THE ENGINE.
///
/// The language binding MUST free the memory associated with this sequence by calling the appropriate 'free' function.
/// For example, all [`OwnedSlice`]s within a [`PipelineResult`] are freed by calling [`cosmoscx_v0_query_pipeline_free_result`].
#[repr(C)]
pub struct OwnedSlice<T> {
    data: *mut T,
    len: usize,
}

impl<T> OwnedSlice<T> {
    pub const EMPTY: OwnedSlice<T> = OwnedSlice {
        data: std::ptr::null_mut(),
        len: 0,
    };

    pub fn into_boxed_slice(self) -> Option<Box<[T]>> {
        let me = mem::ManuallyDrop::new(self);
        if me.len == 0 || me.data.is_null() {
            None
        } else {
            // Don't drop the original OwnedSlice.
            let slice = std::ptr::slice_from_raw_parts_mut(me.data, me.len);
            unsafe { Some(Box::from_raw(slice)) }
        }
    }
}

impl<T> From<Box<[T]>> for OwnedSlice<T> {
    fn from(value: Box<[T]>) -> Self {
        let len = value.len();
        if len == 0 {
            // Don't use the pointer from an empty boxed slice.
            Self::EMPTY
        } else {
            let data = Box::into_raw(value) as *mut _;
            tracing::trace!(ptr = ?data, len = len, typ = std::any::type_name::<OwnedSlice<T>>(), from = std::any::type_name::<Box<[T]>>(), "allocated");
            Self { data, len }
        }
    }
}

impl<T> From<Vec<T>> for OwnedSlice<T> {
    fn from(value: Vec<T>) -> Self {
        value.into_boxed_slice().into()
    }
}

impl<T> From<Option<T>> for OwnedSlice<T>
where
    OwnedSlice<T>: From<T>,
{
    fn from(value: Option<T>) -> Self {
        value.map(|v| v.into()).unwrap_or(OwnedSlice::EMPTY)
    }
}

impl<T> Drop for OwnedSlice<T> {
    fn drop(&mut self) {
        let slice = std::mem::replace(self, OwnedSlice::EMPTY);

        if let Some(boxed) = slice.into_boxed_slice() {
            let len = boxed.len();
            let data = boxed.as_ptr();
            tracing::trace!(ptr = ?data, len = len, typ = std::any::type_name::<OwnedSlice<T>>(), "freeing");
            drop(boxed);
        }
    }
}

pub type OwnedString = OwnedSlice<u8>;

impl OwnedString {
    /// Converts the Owned String back into a String, using the same pointer
    pub unsafe fn into_string(self) -> crate::Result<Option<String>> {
        #[cfg(debug_assertions)]
        let original_addr = self.data as *const u8;

        let Some(slice) = self.into_boxed_slice() else {
            return Ok(None);
        };
        let vec = Vec::from(slice); // This just adds a "capacity" field (on the stack) which is equal to length.
        let string = String::from_utf8(vec).map_err(|_| ErrorKind::InvalidUtf8String)?; // This just checks the characters and then wraps the vec in a String.

        // Validate that the string refers to the same pointer
        debug_assert_eq!(original_addr, string.as_ptr());

        Ok(Some(string))
    }
}

impl From<String> for OwnedString {
    fn from(value: String) -> Self {
        value.into_boxed_str().into_boxed_bytes().into()
    }
}

impl<T> From<Option<T>> for OwnedString
where
    OwnedString: From<T>,
{
    fn from(value: Option<T>) -> Self {
        value.map(|v| v.into()).unwrap_or(OwnedString::EMPTY)
    }
}
