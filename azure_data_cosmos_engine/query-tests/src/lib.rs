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
        count_no_items,
        max_price,
        max_no_items,
        min_price,
        min_no_items,
        average_price,
        average_no_items,
        sum_price,
        sum_no_items,
    },
}
