// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

//! Diagnostics-related functions, such as enabling and configuring tracing.

use tracing_subscriber::EnvFilter;

/// Enables built-in tracing for the Cosmos Client Engine.
///
/// This is an early version of the tracing API and is subject to change.
/// For now, it activates the default console tracing in [`tracing_subscriber::fmt`](fn@tracing_subscriber::fmt) and enables the [`EnvFilter`](`tracing_subscriber::EnvFilter`) using the `COSMOSCX_LOG` environment variable.
///
/// Once enabled in this way, tracing cannot be disabled.
#[no_mangle]
pub extern "C" fn cosmoscx_v0_tracing_enable() {
    // Ignore failures
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_env("COSMOSCX_LOG"))
        .try_init();
}
