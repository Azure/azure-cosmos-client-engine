// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

#[cfg(test)]
#[macro_use]
mod runner;

#[cfg(test)]
baseline_tests! {
    order_by {
        streaming_1,
    },
    vector {
        quantized_cosine,
        flat_euclidean,
        diskann_dotproduct,
    },
    aggregates {
        count_all,
        count_with_where,
    },
}
