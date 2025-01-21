//! Defines the C API for the Cosmos Client Engine
//!
//! NOTE: All Cosmos DB Client Engine functions are prefixed with `cosmoscx_` to ensure they don't conflict with any other APIs the application may be referencing.

use std::ffi::CStr;

const C_VERSION: &CStr = const {
    // We need a const CStr to return from coscx_version, but env! only returns a &str
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

/// Returns the version of the Cosmos Client Engine in use.
#[no_mangle]
extern "C" fn cosmoscx_version() -> *const std::ffi::c_char {
    C_VERSION.as_ptr()
}
