#[cfg(feature = "c_api")]
pub mod c_api;

pub const VERSION: &[u8] = env!("CARGO_PKG_VERSION").as_bytes();
