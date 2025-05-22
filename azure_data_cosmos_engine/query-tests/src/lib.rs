// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

#[cfg(test)]
#[macro_use]
mod runner;

#[cfg(test)]
baseline_tests! {
    order_by {
        streaming_1,
    }
}
