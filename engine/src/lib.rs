#[cfg(feature = "c_api")]
pub mod c_api;

#[cfg(feature = "python")]
mod python;

#[allow(dead_code)]
const VERSION: &[u8] = env!("CARGO_PKG_VERSION").as_bytes();
