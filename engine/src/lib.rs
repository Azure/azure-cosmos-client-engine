// Warnings are errors when building on CI.
#![cfg_attr(not(debug_assertions), deny(warnings))]

#[cfg(feature = "c_api")]
pub mod c_api;

#[cfg(feature = "python")]
mod python;

mod error;

pub(crate) use error::Result;
pub use error::{Error, ErrorKind};

pub mod query;

#[allow(dead_code)]
const VERSION: &[u8] = version().as_bytes();

pub const fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
