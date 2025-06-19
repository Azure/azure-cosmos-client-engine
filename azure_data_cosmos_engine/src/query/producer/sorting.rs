use std::cmp::Ordering;
use std::fmt::Debug;

use crate::{
    query::{QueryClauseItem, QueryResult, SortOrder},
    ErrorKind,
};

pub struct Sorting(pub Vec<SortOrder>);

/// Represents the result of the sorting comparison.
///
/// We use this instead of [`Ordering`] because we may be ordering fields either ascending or descending.
/// This type handles that, so returning values like `Less`, `Greater`, or `Equal` isn't as clear, and we want a clear distinction between the two.
#[derive(Debug, PartialEq, Eq)]
pub enum SortResult {
    /// The left item should be sorted before the right item.
    LeftBeforeRight,

    /// The right item should be sorted before the left item.
    RightBeforeLeft,

    /// The two items are equal in terms of sorting.
    Equal,
}

impl Sorting {
    /// Compares two items based on the sorting order defined in this `Sorting` instance.
    ///
    /// The `left` and `right` parameters are optional, allowing for comparisons where one or both items may be absent.
    pub fn compare<T: Debug, I: QueryClauseItem>(
        &self,
        left: Option<&QueryResult<T, I>>,
        right: Option<&QueryResult<T, I>>,
    ) -> crate::Result<SortResult> {
        let (left, right) = match (left, right) {
            (Some(left), Some(right)) => (left, right),

            // "Empty" partitions sort before non-empty partitions, because they need to cause iteration to stop so we can get more data.
            (None, Some(_)) => return Ok(SortResult::LeftBeforeRight),
            (Some(_), None) => return Ok(SortResult::RightBeforeLeft),
            (None, None) => {
                return Ok(SortResult::Equal);
            }
        };

        if left.order_by_items.len() != right.order_by_items.len() {
            return Err(ErrorKind::InvalidGatewayResponse
                .with_message("items have inconsistent numbers of order by items"));
        }

        if left.order_by_items.len() != self.0.len() {
            return Err(ErrorKind::InvalidGatewayResponse
                .with_message("items have inconsistent numbers of order by items"));
        }

        let items = left
            .order_by_items
            .iter()
            .zip(right.order_by_items.iter())
            .zip(self.0.iter());

        for ((left, right), ordering) in items {
            let order = left.compare(right)?;
            match (ordering, order) {
                (SortOrder::Ascending, Ordering::Less) => return Ok(SortResult::LeftBeforeRight),
                (SortOrder::Ascending, Ordering::Greater) => {
                    return Ok(SortResult::RightBeforeLeft)
                }
                (SortOrder::Descending, Ordering::Less) => return Ok(SortResult::RightBeforeLeft),
                (SortOrder::Descending, Ordering::Greater) => {
                    return Ok(SortResult::LeftBeforeRight)
                }

                // If the order is equal, we continue to the next item.
                (_, Ordering::Equal) => {}
            }
        }

        // The values are equal. Our caller will have to pick a tiebreaker.
        Ok(SortResult::Equal)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::value::RawValue;

    use crate::{
        query::{
            producer::sorting::{SortResult, Sorting},
            JsonQueryClauseItem, QueryResult,
        },
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
        let sorting = Sorting(vec![
            crate::query::SortOrder::Ascending,
            crate::query::SortOrder::Descending,
        ]);
        assert_eq!(
            SortResult::LeftBeforeRight,
            sorting.compare(Some(&left), Some(&right)).unwrap()
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
        let sorting = Sorting(vec![
            crate::query::SortOrder::Ascending,
            crate::query::SortOrder::Descending,
        ]);
        assert_eq!(
            SortResult::Equal,
            sorting.compare(Some(&left), Some(&right)).unwrap()
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
        let sorting = Sorting(vec![
            crate::query::SortOrder::Ascending,
            crate::query::SortOrder::Descending,
        ]);
        assert_eq!(
            SortResult::LeftBeforeRight,
            sorting.compare(None, Some(&non_empty)).unwrap()
        );
        assert_eq!(
            SortResult::RightBeforeLeft,
            sorting.compare(Some(&non_empty), None).unwrap()
        );
        assert_eq!(
            SortResult::Equal,
            sorting
                .compare::<Box<RawValue>, JsonQueryClauseItem>(None, None)
                .unwrap()
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
        let sorting = Sorting(vec![
            crate::query::SortOrder::Ascending,
            crate::query::SortOrder::Descending,
        ]);
        let err = sorting.compare(Some(&left), Some(&right)).unwrap_err();
        assert_eq!(ErrorKind::InvalidGatewayResponse, err.kind());
    }
}
