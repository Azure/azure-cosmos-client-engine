// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

// Warnings are errors when building on CI.
#![cfg_attr(not(debug_assertions), deny(warnings))]

macro_rules! make_cstr {
    ($s: expr) => {
        unsafe { std::ffi::CStr::from_bytes_with_nul_unchecked(concat!($s, "\0").as_bytes()) }
    };
}

mod error;

pub(crate) use error::Result;
pub use error::{Error, ErrorKind};

pub mod query;

/// The version of the Cosmos Client Engine, exposed as a [`CStr`](std::ffi::CStr) so that it can easily be exposed by C-based FFI as well consumed by Rust (via [`CStr::to_str`](std::ffi::CStr::to_str).
pub static VERSION: &std::ffi::CStr = make_cstr!(env!("CARGO_PKG_VERSION"));
