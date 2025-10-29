// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use std::{cmp::Ordering, sync::Arc};

use crate::{
    query::{QueryClauseItem, QueryResult, SortOrder},
    ErrorKind,
};

pub struct SortableResult(Sorting, QueryResult);

impl PartialEq for SortableResult {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for SortableResult {}

impl PartialOrd for SortableResult {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SortableResult {
    fn cmp(&self, other: &Self) -> Ordering {
        // Unless the gateway provides invalid data, this shouldn't fail.
        self.0
            .compare(Some(&self.1.order_by_items), Some(&other.1.order_by_items))
            .expect("Sorting should not fail")
    }
}

impl SortableResult {
    pub fn new(sorting: Sorting, result: QueryResult) -> Self {
        Self(sorting, result)
    }
}

impl From<SortableResult> for QueryResult {
    fn from(value: SortableResult) -> Self {
        value.1
    }
}

#[derive(Debug, Clone)]
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
    pub fn compare(
        &self,
        left: Option<&[QueryClauseItem]>,
        right: Option<&[QueryClauseItem]>,
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
        query::{producer::sorting::Sorting, QueryClauseItem, QueryResult},
        ErrorKind,
    };

    #[test]
    pub fn compare_query_results_different() {
        let left = QueryResult {
            order_by_items: vec![
                QueryClauseItem::from_value(serde_json::json!(1)),
                QueryClauseItem::from_value(serde_json::json!("zzzz")),
            ],
            ..Default::default()
        };
        let right = QueryResult {
            order_by_items: vec![
                QueryClauseItem::from_value(serde_json::json!(1)),
                QueryClauseItem::from_value(serde_json::json!("yyyy")),
            ],
            ..Default::default()
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
                QueryClauseItem::from_value(serde_json::json!(1)),
                QueryClauseItem::from_value(serde_json::json!("zzzz")),
            ],
            ..Default::default()
        };
        let right = QueryResult {
            order_by_items: vec![
                QueryClauseItem::from_value(serde_json::json!(1)),
                QueryClauseItem::from_value(serde_json::json!("zzzz")),
            ],
            ..Default::default()
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
                QueryClauseItem::from_value(serde_json::json!(1)),
                QueryClauseItem::from_value(serde_json::json!("zzzz")),
            ],
            ..Default::default()
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
        assert_eq!(Ordering::Equal, sorting.compare(None, None).unwrap());
    }

    #[test]
    pub fn compare_query_results_inconsistent() {
        let left = QueryResult {
            order_by_items: vec![QueryClauseItem::from_value(serde_json::json!(1))],
            ..Default::default()
        };
        let right = QueryResult {
            order_by_items: vec![
                QueryClauseItem::from_value(serde_json::json!(1)),
                QueryClauseItem::from_value(serde_json::json!("zzzz")),
            ],
            ..Default::default()
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
