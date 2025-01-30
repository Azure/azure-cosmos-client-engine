#[cfg(feature = "c_api")]
pub mod c_api;

#[cfg(feature = "python")]
mod python;

mod error;

pub(crate) use error::Result;
pub use error::{Error, ErrorKind};

pub mod query;

#[allow(dead_code)]
const VERSION: &[u8] = env!("CARGO_PKG_VERSION").as_bytes();
