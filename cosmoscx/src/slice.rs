// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

//! FFI-safe types for providing slices of memory.

use std::{marker::PhantomData, mem};

use azure_data_cosmos_engine::ErrorKind;

/// Represents a contiguous sequence of objects OWNED BY THE CALLING CODE.
///
/// The language binding owns this memory. It must keep the memory valid for the duration of any function call that receives it.
/// For example, the [`Slice`]s passed to [`cosmoscx_v0_query_pipeline_create`](super::pipeline::cosmoscx_v0_query_pipeline_create) must remain valid until that function returns.
/// After the function returns, the language binding may free the memory.
/// This lifetime is represented by the lifetime parameter `'a`, which should prohibit Rust code from storing the value.
///
/// The C representation of this struct is identical to [`OwnedSlice`], the only difference is that this type indicates that the language binding owns this memory.
/// The language binding is responsible for ensuring the underlying `data` pointer and `len` are correct and the data is properly aligned such that the `data` pointer is a valid C-style array of `T` values.
#[repr(C)]
pub struct Slice<'a, T> {
    data: *const T,
    len: usize,
    _phantom: PhantomData<&'a T>,
}

impl<'a, T> Slice<'a, T> {
    /// An empty slice, represented by a null data pointer and `0` length.
    pub const EMPTY: Slice<'a, T> = Slice {
        data: std::ptr::null(),
        len: 0,
        _phantom: PhantomData,
    };

    /// Returns a `&[T]` pointing to the underlying data.
    ///
    /// If the underlying pointer is null, this returns `Ok(None)`.
    /// If the underlying string data is not valid UTF-8, this returns `Err(..)`.
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

/// A [`Slice`] of `u8` values, which must ALSO be a valid UTF-8 string.
///
/// The language binding owns this memory. It must keep the memory valid for the duration of any function call that receives it.
/// For example, the [`Slice`]s passed to [`cosmoscx_v0_query_pipeline_create`](super::pipeline::cosmoscx_v0_query_pipeline_create) must remain valid until that function returns.
/// After the function returns, the language binding may free the memory.
/// This lifetime is represented by the lifetime parameter `'a`, which should prohibit Rust code from storing the value.
///
/// This is a "C-safe" type wrapping the equivalent of a rust [`&str`](primitive@str).
pub type Str<'a> = Slice<'a, u8>;

impl<'a> Str<'a> {
    /// Returns a `&str` pointing to the underlying string data.
    ///
    /// If the underlying pointer is null, this returns `Ok(None)`.
    /// If the underlying string data is not valid UTF-8, this returns `Err(..)`.
    pub unsafe fn as_str(&self) -> Result<Option<&'a str>, azure_data_cosmos_engine::Error> {
        let Some(slice) = self.as_slice() else {
            return Ok(None);
        };
        Some(std::str::from_utf8(slice).map_err(|_| ErrorKind::InvalidUtf8String.into()))
            .transpose()
    }

    /// Creates a copy of the underlying string data
    pub unsafe fn into_string(&self) -> Result<Option<String>, azure_data_cosmos_engine::Error> {
        self.as_str().map(|o| o.map(|s| s.to_string()))
    }
}

impl<'a> From<&'a str> for Str<'a> {
    fn from(value: &'a str) -> Self {
        Slice::from(value.as_bytes())
    }
}

/// Represents a contiguous sequence of objects OWNED BY THE ENGINE.
///
/// The language binding MUST free the memory associated with this sequence by calling the appropriate 'free' function.
/// For example, all [`OwnedSlice`]s within a [`PipelineResponse`](azure_data_cosmos_engine::query::PipelineResponse) are freed by calling [`cosmoscx_v0_query_pipeline_free_result`](super::pipeline::cosmoscx_v0_query_pipeline_free_result).
///
/// The C representation of this struct is:
///
/// ```
/// struct {
///   const void *data; // A pointer to the first item in the slice
///   intptr_t len; // The number of items in the slice.
/// };
/// ```
///
/// The `data` pointer is guaranteed to point to a contiguous sequence of `T` values.
/// Each `T` value will be properly aligned.
/// Thus, the `data` pointer can be treated as a C-style array of length `len`.
#[repr(C)]
pub struct OwnedSlice<T> {
    data: *mut T,
    len: usize,
}

impl<T> OwnedSlice<T> {
    /// An empty slice, represented by a null data pointer and `0` length.
    pub const EMPTY: OwnedSlice<T> = OwnedSlice {
        data: std::ptr::null_mut(),
        len: 0,
    };

    /// Consumes this `OwnedSlice` and returns it as an `Option<Box<[T]>>`.
    ///
    /// This method is intended to take the FFI-safe `OwnedSlice<T>` and transform it "back" into a Rust-managed type.
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
    /// Consumes a Rust-managed boxed slice and converts it into an [`OwnedSlice`].
    ///
    /// This is the way to convert a Rust-managed slice into an FFI-safe [`OwnedSlice`] to return to the language binding caller.
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
    /// Consumes a Rust-managed [`Vec`] and converts it into an [`OwnedSlice`].
    ///
    /// This is the way to convert a Rust-managed [`Vec`] into an FFI-safe [`OwnedSlice`] to return to the language binding caller.
    fn from(value: Vec<T>) -> Self {
        value.into_boxed_slice().into()
    }
}

impl<T> From<Option<T>> for OwnedSlice<T>
where
    OwnedSlice<T>: From<T>,
{
    /// Consumes an `Option<T>` (where `T` is convertible to an [`OwnedSlice`]) and converts it into an [`OwnedSlice`].
    ///
    /// This allows the caller to transform a Rust optional type into a potentially-nullable pointer.
    /// If the value provided is `Some`, this returns an [`OwnedSlice`] using the [`OwnedSlice::from`] method.
    /// If the value is `None`, this returns [`OwnedSlice::EMPTY`]
    fn from(value: Option<T>) -> Self {
        value.map(|v| v.into()).unwrap_or(OwnedSlice::EMPTY)
    }
}

impl<T> Drop for OwnedSlice<T> {
    /// Drops the `OwnedSlice`, AND frees the underlying Rust-managed memory.
    ///
    /// This effectively calls [`OwnedSlice::into_boxed_slice`] and then drops the boxed slice.
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

/// Represents a contiguous sequence of valid UTF-8 bytes OWNED BY THE ENGINE.
///
/// The language binding MUST free the memory associated with this sequence by calling the appropriate 'free' function.
/// For example, all [`OwnedSlice`]s within a [`PipelineResponse`](azure_data_cosmos_engine::query::PipelineResponse) are freed by calling [`cosmoscx_v0_query_pipeline_free_result`](super::pipeline::cosmoscx_v0_query_pipeline_free_result).
pub type OwnedString = OwnedSlice<u8>;

impl OwnedString {
    /// Consumes the [`OwnedString`] and returns a [`String`] representing the same data.
    ///
    /// If the underlying slice pointer is null, this returns `Ok(None)`.
    /// If the underlying bytes are not valid UTF-8, this returns `Err` (this is unlikely as the string _should_ have been created by Rust).
    pub unsafe fn into_string(self) -> Result<Option<String>, azure_data_cosmos_engine::Error> {
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
    /// Consumes the provided [`String`] and converts it in to an FFI-safe [`OwnedString`].
    fn from(value: String) -> Self {
        value.into_boxed_str().into_boxed_bytes().into()
    }
}

impl<T> From<Option<T>> for OwnedString
where
    OwnedString: From<T>,
{
    /// Consumes an `Option<T>` (where `T` is convertible to an [`OwnedString`]) and converts it into an [`OwnedString`].
    ///
    /// This allows the caller to transform a Rust optional type into a potentially-nullable pointer.
    /// If the value provided is `Some`, this returns an [`OwnedString`] using the [`OwnedString::from`] method.
    /// If the value is `None`, this returns [`OwnedString::EMPTY`]
    fn from(value: Option<T>) -> Self {
        value.map(|v| v.into()).unwrap_or(OwnedString::EMPTY)
    }
}
