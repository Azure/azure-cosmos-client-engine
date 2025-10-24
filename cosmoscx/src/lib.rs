// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

//! Defines the C API for the Cosmos Client Engine
//!
//! API functions can be found throughout the Rust modules in this crate.
//! However, despite being nested in Rust modules, C callers can call the APIs using only the name of the function.
//!
//! NOTE: All Cosmos DB Client Engine functions are prefixed with `cosmoscx_` to ensure they don't conflict with any other APIs the application may be referencing.

use azure_data_cosmos_engine::query::SUPPORTED_FEATURES;

pub mod diag;
pub mod pipeline;
pub mod result;
pub mod slice;

unsafe fn free<T>(ptr: *mut T) {
    // SAFETY: We have to trust that the caller is giving us a valid pipeline result from calling "run"
    let owned = unsafe { Box::from_raw(ptr) };
    tracing::trace!(?ptr, typ = std::any::type_name_of_val(&owned), "freeing");
    drop(owned);
}

/// Returns the version of the Cosmos Client Engine in use.
#[no_mangle]
pub extern "C" fn cosmoscx_version() -> *const std::ffi::c_char {
    azure_data_cosmos_engine::VERSION.as_ptr()
}

/// Returns a string that describes the query features supported by the Cosmos Client Engine.
///
/// This string is suitable to be sent as the value for the `x-ms-cosmos-supported-query-features` header in a query plan request.
#[no_mangle]
pub extern "C" fn cosmoscx_v0_query_supported_features() -> *const std::ffi::c_char {
    SUPPORTED_FEATURES.as_cstr().as_ptr()
}

#[no_mangle]
/// cbindgen:ignore
pub static BUILD_IDENTIFIER: &str = env!("BUILD_IDENTIFIER");

// For testing panic behavior in wrappers.
/// cbindgen:ignore
#[cfg(debug_assertions)]
#[no_mangle]
pub extern "C" fn cosmoscx_v0_panic() {
    panic!("This is a test panic from the Cosmos Client Engine");
}
