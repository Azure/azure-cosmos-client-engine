// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use std::env;

pub fn main() {
    let build_id = format!(
        "$Id: {}, Version: {}, Commit: {}, Branch: {}, Build ID: {}, Build Number: {}, Timestamp: {}$",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
        option_env!("BUILD_SOURCEVERSION").unwrap_or("unknown"),
        option_env!("BUILD_SOURCEBRANCH").unwrap_or("unknown"),
        option_env!("BUILD_BUILDID").unwrap_or("unknown"),
        option_env!("BUILD_BUILDNUMBER").unwrap_or("unknown"),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    println!("cargo:rustc-env=BUILD_IDENTIFIER={}", build_id);
}
