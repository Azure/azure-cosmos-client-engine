// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use std::{cmp::Ordering, fmt::Debug, sync::Arc};

use crate::{
    query::{QueryClauseItem, QueryResult, SortOrder},
    ErrorKind,
};

// If we ever want to make this Send, to support parallel processing, we can use `Arc` instead of `Rc`.
pub struct SortableResult<T: Debug, I: QueryClauseItem>(Sorting, QueryResult<T, I>);

impl<T: Debug, I: QueryClauseItem> PartialEq for SortableResult<T, I> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl<T: Debug, I: QueryClauseItem> Eq for SortableResult<T, I> {}

impl<T: Debug, I: QueryClauseItem> PartialOrd for SortableResult<T, I> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: Debug, I: QueryClauseItem> Ord for SortableResult<T, I> {
    fn cmp(&self, other: &Self) -> Ordering {
        // Unless the gateway provides invalid data, this shouldn't fail.
        self.0
            .compare(Some(&self.1.order_by_items), Some(&other.1.order_by_items))
            .expect("Sorting should not fail")
    }
}

impl<T: Debug, I: QueryClauseItem> SortableResult<T, I> {
    pub fn new(sorting: Sorting, result: QueryResult<T, I>) -> Self {
        Self(sorting, result)
    }
}

impl<T: Debug, I: QueryClauseItem> From<SortableResult<T, I>> for QueryResult<T, I> {
    fn from(value: SortableResult<T, I>) -> Self {
        value.1
    }
}

#[derive(Clone)]
pub struct Sorting(Arc<[SortOrder]>);

impl Sorting {
    pub fn new(ordering: Vec<SortOrder>) -> Self {
        Self(Arc::from(ordering))
    }

    /// Compares two items based on the sorting order defined in this `Sorting` instance.
    ///
    /// This ALWAYS returns an ordering based on sorting from LARGEST to SMALLEST, meaning that the first item in the list is greater than the second item.
    /// We do this because we use a [`BinaryHeap`](std::collections::BinaryHeap) to sort items, which is a max-heap.
    ///
    /// In other words, we return an [`Ordering`] such that a DESCENDING sort of the items will result in the user's desired sort order.
    ///
    /// The `left` and `right` parameters are optional, allowing for comparisons where one or both items may be absent.
    pub fn compare<I: QueryClauseItem>(
        &self,
        left: Option<&[I]>,
        right: Option<&[I]>,
    ) -> crate::Result<Ordering> {
        let (left, right) = match (left, right) {
            (Some(left), Some(right)) => (left, right),

            // "Empty" partitions sort before non-empty partitions, because they need to cause iteration to stop so we can get more data.
            (None, Some(_)) => return Ok(Ordering::Greater),
            (Some(_), None) => return Ok(Ordering::Less),
            (None, None) => {
                return Ok(Ordering::Equal);
            }
        };

        if left.len() != right.len() {
            return Err(ErrorKind::InvalidGatewayResponse
                .with_message("items have inconsistent numbers of order by items"));
        }

        if left.len() != self.0.len() {
            return Err(ErrorKind::InvalidGatewayResponse
                .with_message("items have inconsistent numbers of order by items"));
        }

        let items = left.iter().zip(right.iter()).zip(self.0.iter());

        for ((left, right), ordering) in items {
            let order = left.compare(right)?;
            match (ordering, order) {
                (SortOrder::Ascending, Ordering::Less) => return Ok(Ordering::Greater),
                (SortOrder::Ascending, Ordering::Greater) => return Ok(Ordering::Less),
                (SortOrder::Descending, Ordering::Less) => return Ok(Ordering::Less),
                (SortOrder::Descending, Ordering::Greater) => return Ok(Ordering::Greater),

                // If the order is equal, we continue to the next item.
                (_, Ordering::Equal) => {}
            }
        }

        // The values are equal. Our caller will have to pick a tiebreaker.
        Ok(Ordering::Equal)
    }
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;

    use crate::{
        query::{producer::sorting::Sorting, JsonQueryClauseItem, QueryResult},
        ErrorKind,
    };

    #[test]
    pub fn compare_query_results_different() {
        let left = QueryResult {
            order_by_items: vec![
                JsonQueryClauseItem {
                    item: Some(serde_json::json!(1)),
                },
                JsonQueryClauseItem {
                    item: Some(serde_json::json!("zzzz")),
                },
            ],
            group_by_items: vec![],
            payload: serde_json::value::RawValue::from_string(r#"{"a":1}"#.to_string()).unwrap(),
        };
        let right = QueryResult {
            order_by_items: vec![
                JsonQueryClauseItem {
                    item: Some(serde_json::json!(1)),
                },
                JsonQueryClauseItem {
                    item: Some(serde_json::json!("yyyy")),
                },
            ],
            group_by_items: vec![],
            payload: serde_json::value::RawValue::from_string(r#"{"a":1}"#.to_string()).unwrap(),
        };
        let sorting = Sorting::new(vec![
            crate::query::SortOrder::Ascending,
            crate::query::SortOrder::Descending,
        ]);
        assert_eq!(
            Ordering::Greater,
            sorting
                .compare(Some(&left.order_by_items), Some(&right.order_by_items))
                .unwrap()
        );
    }

    #[test]
    pub fn compare_query_results_identical() {
        let left = QueryResult {
            order_by_items: vec![
                JsonQueryClauseItem {
                    item: Some(serde_json::json!(1)),
                },
                JsonQueryClauseItem {
                    item: Some(serde_json::json!("zzzz")),
                },
            ],
            group_by_items: vec![],
            payload: serde_json::value::RawValue::from_string(r#"{"a":1}"#.to_string()).unwrap(),
        };
        let right = QueryResult {
            order_by_items: vec![
                JsonQueryClauseItem {
                    item: Some(serde_json::json!(1)),
                },
                JsonQueryClauseItem {
                    item: Some(serde_json::json!("zzzz")),
                },
            ],
            group_by_items: vec![],
            payload: serde_json::value::RawValue::from_string(r#"{"a":1}"#.to_string()).unwrap(),
        };
        let sorting = Sorting::new(vec![
            crate::query::SortOrder::Ascending,
            crate::query::SortOrder::Descending,
        ]);
        assert_eq!(
            Ordering::Equal,
            sorting
                .compare(Some(&left.order_by_items), Some(&right.order_by_items))
                .unwrap()
        );
    }

    #[test]
    pub fn compare_with_empty() {
        let non_empty = QueryResult {
            order_by_items: vec![
                JsonQueryClauseItem {
                    item: Some(serde_json::json!(1)),
                },
                JsonQueryClauseItem {
                    item: Some(serde_json::json!("zzzz")),
                },
            ],
            group_by_items: vec![],
            payload: serde_json::value::RawValue::from_string(r#"{"a":1}"#.to_string()).unwrap(),
        };
        let sorting = Sorting::new(vec![
            crate::query::SortOrder::Ascending,
            crate::query::SortOrder::Descending,
        ]);
        assert_eq!(
            Ordering::Greater,
            sorting
                .compare(None, Some(&non_empty.order_by_items))
                .unwrap()
        );
        assert_eq!(
            Ordering::Less,
            sorting
                .compare(Some(&non_empty.order_by_items), None)
                .unwrap()
        );
        assert_eq!(
            Ordering::Equal,
            sorting.compare::<JsonQueryClauseItem>(None, None).unwrap()
        );
    }

    #[test]
    pub fn compare_query_results_inconsistent() {
        let left = QueryResult {
            order_by_items: vec![JsonQueryClauseItem {
                item: Some(serde_json::json!(1)),
            }],
            group_by_items: vec![],
            payload: serde_json::value::RawValue::from_string(r#"{"a":1}"#.to_string()).unwrap(),
        };
        let right = QueryResult {
            order_by_items: vec![
                JsonQueryClauseItem {
                    item: Some(serde_json::json!(1)),
                },
                JsonQueryClauseItem {
                    item: Some(serde_json::json!("zzzz")),
                },
            ],
            group_by_items: vec![],
            payload: serde_json::value::RawValue::from_string(r#"{"a":1}"#.to_string()).unwrap(),
        };
        let sorting = Sorting::new(vec![
            crate::query::SortOrder::Ascending,
            crate::query::SortOrder::Descending,
        ]);
        let err = sorting
            .compare(Some(&left.order_by_items), Some(&right.order_by_items))
            .unwrap_err();
        assert_eq!(ErrorKind::InvalidGatewayResponse, err.kind());
    }
}
