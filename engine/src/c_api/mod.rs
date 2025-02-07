//! Defines the C API for the Cosmos Client Engine
//!
//! NOTE: All Cosmos DB Client Engine functions are prefixed with `cosmoscx_` to ensure they don't conflict with any other APIs the application may be referencing.

use std::ffi::CStr;

use tracing_subscriber::EnvFilter;

use crate::query;

pub mod pipeline;
pub mod result;
pub mod slice;

const C_VERSION: &CStr = const {
    // We need a const CStr to return from cosmoscx_version, but env! only returns a &str
    // This all gets interpreted by the compiler at compile time and embedded into the binary
    const VERSION: &[u8] = env!("CARGO_PKG_VERSION").as_bytes();
    const BYTES: [u8; VERSION.len() + 1] = const {
        let mut bytes = [0u8; VERSION.len() + 1];
        let mut i = 0;
        while i < VERSION.len() {
            bytes[i] = VERSION[i];
            i += 1;
        }
        bytes
    };

    match CStr::from_bytes_with_nul(&BYTES) {
        Ok(s) => s,
        Err(_) => panic!("version string contains null bytes"),
    }
};

unsafe fn free<T>(ptr: *mut T) {
    // SAFETY: We have to trust that the caller is giving us a valid pipeline result from calling "next_batch"
    let owned = unsafe { Box::from_raw(ptr) };
    tracing::trace!(?ptr, typ = std::any::type_name_of_val(&owned), "freeing");
    drop(owned);
}

/// Returns the version of the Cosmos Client Engine in use.
#[no_mangle]
extern "C" fn cosmoscx_version() -> *const std::ffi::c_char {
    C_VERSION.as_ptr()
}

const C_SUPPORTED_FEATURES: &CStr = const {
    const BYTES: [u8; query::SUPPORTED_FEATURES_STRING.len() + 1] = const {
        let mut bytes = [0u8; query::SUPPORTED_FEATURES_STRING.len() + 1];
        let mut i = 0;
        while i < query::SUPPORTED_FEATURES_STRING.len() {
            bytes[i] = query::SUPPORTED_FEATURES_STRING.as_bytes()[i];
            i += 1;
        }
        bytes
    };

    match CStr::from_bytes_with_nul(&BYTES) {
        Ok(s) => s,
        Err(_) => panic!("supported features string contains null bytes"),
    }
};

/// Returns a string that describes the query features supported by the Cosmos Client Engine.
///
/// This string is suitable to be sent as the value for the `x-ms-cosmos-supported-query-features` header in a query plan request.
#[no_mangle]
extern "C" fn cosmoscx_v0_query_supported_features() -> *const std::ffi::c_char {
    C_SUPPORTED_FEATURES.as_ptr()
}

/// Enables built-in tracing for the Cosmos Client Engine.
///
/// This is an early version of the tracing API and is subject to change.
/// For now, it activates the default console tracing in [`tracing_subscriber::fmt`] and enables the [`EnvFilter`](`tracing_subscriber::EnvFilter`) using the `COSMOSCX_LOG` environment variable.
///
/// Once enabled in this way, tracing cannot be disabled.
#[no_mangle]
extern "C" fn cosmoscx_v0_tracing_enable() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_env("COSMOSCX_LOG"))
        .init();
}
