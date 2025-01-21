#[cfg(feature = "c_api")]
pub mod c_api;

#[cfg(feature = "python")]
mod python;

pub const VERSION: &[u8] = env!("CARGO_PKG_VERSION").as_bytes();
