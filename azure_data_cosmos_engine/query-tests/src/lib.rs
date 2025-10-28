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
        average_no_items,
        average_price,
        average_where,
        count_all,
        count_no_items,
        count_where,
        max_no_items,
        max_price,
        max_where,
        min_no_items,
        min_price,
        min_where,
        sum_no_items,
        sum_price,
        sum_where,
    },
    hybrid {
        top_10_by_fulltext_rank,
        offset_limit,
        top_20_rrf,
        top_10_rrf,
        offset_limit_rrf,
        order_by_rrf_unfiltered,
        offset_limit_rrf_unfiltered,
        offset_limit_rrf_ft_with_vector,
    },
}
