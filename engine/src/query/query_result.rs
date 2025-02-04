use serde::{Deserialize, Deserializer};

use crate::ErrorKind;

use super::SortOrder;

/// Represents the result of a rewritten query.
///
/// When we generate a query plan, the gateway rewrites the query so that it can be properly executed against each partition.
/// For example, order by items are collected into a well-known property with a well-known format so that the pipeline can easily access them.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryResult<T = Box<serde_json::value::RawValue>> {
    #[serde(default)]
    group_by_items: Vec<QueryClauseItem>,

    #[serde(default)]
    order_by_items: Vec<QueryClauseItem>,

    payload: T,
}

impl<T> QueryResult<T> {
    pub fn new(
        group_by_items: Vec<QueryClauseItem>,
        order_by_items: Vec<QueryClauseItem>,
        payload: T,
    ) -> Self {
        Self {
            group_by_items,
            order_by_items,
            payload,
        }
    }

    /// Creates a [`QueryResult`] from a raw payload.
    ///
    /// This constructor is used when we've identified that the _projections_ of the query haven't been rewritten by the query planner.
    /// For example, if the query doesn't include any `ORDER BY` or `GROUP BY` clauses, the payload will be the same as the original query.
    /// However, if the query DOES include `ORDER BY` or `GROUP BY` clauses, you should use the implementation of `Deserialize` to directly deserialize the query result received for each partition.
    pub fn from_payload(payload: T) -> Self {
        // NOTE: An empty vec that _stays empty_ is "free" allocation-wise. It's basically just a null pointer.
        Self {
            group_by_items: Vec::new(),
            order_by_items: Vec::new(),
            payload: payload.into(),
        }
    }

    /// Compares two [`QueryResult`]s based on their `ORDER BY` items and the provided sort ordering.
    ///
    /// We can't just implement [`PartialOrd`] here, because we need to accept a sort order and return an error.
    pub fn compare(
        &self,
        other: &QueryResult<T>,
        orderings: &[SortOrder],
    ) -> crate::Result<std::cmp::Ordering> {
        if self.order_by_items.len() != other.order_by_items.len() {
            return Err(ErrorKind::QueryPlanInvalid
                .with_message("items have inconsistent numbers of order by items"));
        }

        if self.order_by_items.len() != orderings.len() {
            return Err(ErrorKind::QueryPlanInvalid
                .with_message("items have inconsistent numbers of order by items"));
        }

        let items = self
            .order_by_items
            .iter()
            .zip(other.order_by_items.iter())
            .zip(orderings.iter());

        for ((left, right), ordering) in items {
            let order = left.compare(right)?;
            let order = match ordering {
                SortOrder::Ascending => order,
                SortOrder::Descending => order.reverse(),
            };

            if order != std::cmp::Ordering::Equal {
                return Ok(order);
            }
        }

        // The values are equal. Our caller will have to pick a tiebreaker.
        Ok(std::cmp::Ordering::Equal)
    }

    pub fn payload(&self) -> &T {
        &self.payload
    }

    pub fn into_payload(self) -> T {
        self.payload
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct QueryClauseItem {
    #[serde(default, deserialize_with = "deserialize_item")]
    pub item: Option<serde_json::Value>,
}

// Based on https://github.com/serde-rs/serde/issues/984#issuecomment-314143738
// This will deserialize a missing field to `None`, a present-but-null field to `Some(serde_json::Value::Null)` and a present-non-null field to `Some(value)`.
fn deserialize_item<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Deserialize::deserialize(deserializer).map(Some)
}

impl QueryClauseItem {
    /// Compares two [`QueryClauseItem`]s based on the ordering rules defined for Cosmos DB.
    ///
    /// We can't just implement [`PartialOrd`] here, because we need to be able to return an error.
    pub fn compare(&self, other: &Self) -> crate::Result<std::cmp::Ordering> {
        let left_ordinal = self.type_ordinal()?;
        let right_ordinal = other.type_ordinal()?;

        if left_ordinal != right_ordinal {
            return Ok(left_ordinal.cmp(&right_ordinal));
        }

        match (&self.item, &other.item) {
            (None, None) => Ok(std::cmp::Ordering::Equal),
            (Some(serde_json::Value::Null), Some(serde_json::Value::Null)) => {
                Ok(std::cmp::Ordering::Equal)
            }
            (Some(serde_json::Value::String(left)), Some(serde_json::Value::String(right))) => {
                Ok(left.cmp(right))
            }
            (Some(serde_json::Value::Bool(left)), Some(serde_json::Value::Bool(right))) => {
                Ok(left.cmp(right))
            }
            (Some(serde_json::Value::Number(left)), Some(serde_json::Value::Number(right))) => {
                // Try integer comparison first. This will fail if either value is not an integer.
                if let (Some(l_int), Some(r_int)) = (left.as_i64(), right.as_i64()) {
                    Ok(l_int.cmp(&r_int))
                } else {
                    // We need to compare as floats.
                    let l = left.as_f64().ok_or_else(|| {
                        ErrorKind::QueryPlanInvalid.with_message("encountered NaN or Infinity while comparing floats")
                    })?;
                    let r = right.as_f64().ok_or_else(|| {
                        ErrorKind::QueryPlanInvalid.with_message("encountered NaN or Infinity while comparing floats")
                    })?;
                    l.partial_cmp(&r).ok_or_else(|| {
                        ErrorKind::QueryPlanInvalid.with_message("encountered NaN or Infinity while comparing floats")
                    })
                }
            }

            // Shouldn't be possible to get here, since we've already checked the type ordinal.
            _ => unreachable!("encountered different types after comparing type ordinal, this shouldn't be possible")
        }
    }
}

impl QueryClauseItem {
    /// Gets the "Type Ordinal" for a given item.
    ///
    /// The Type Ordinal is used to order items of differing types.
    /// If the Type Ordinal is the same, the items are compared using their underlying values.
    ///
    /// Returns an error if a non-primitive value is encountered.
    fn type_ordinal(&self) -> crate::Result<usize> {
        match &self.item {
            None => Ok(0),
            Some(serde_json::Value::Null) => Ok(1),
            Some(serde_json::Value::Bool(_)) => Ok(2),
            // 3 is skipped in the current implementation for both Python and JS.
            Some(serde_json::Value::Number(_)) => Ok(4),
            Some(serde_json::Value::String(_)) => Ok(5),
            _ => {
                Err(ErrorKind::QueryPlanInvalid.with_message("cannot compare non-primitive values"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;

    use super::*;

    #[test]
    pub fn query_result_deserializes_order_by_items_only() {
        const JSON: &str = r#"{"orderByItems":[{"item":1}], "payload": {"a":1}}"#;
        let result: QueryResult = serde_json::from_str(JSON).unwrap();
        assert_eq!(result.group_by_items, vec![]);
        assert_eq!(
            result.order_by_items,
            vec![QueryClauseItem {
                item: Some(serde_json::json!(1))
            }]
        );
        assert_eq!(result.payload.get(), r#"{"a":1}"#);
    }

    #[test]
    pub fn query_result_deserializes_group_by_items_only() {
        const JSON: &str = r#"{"groupByItems":[{"item":"yoot"}], "payload": {"a":1}}"#;
        let result: QueryResult = serde_json::from_str(JSON).unwrap();
        assert_eq!(
            result.group_by_items,
            vec![QueryClauseItem {
                item: Some(serde_json::json!("yoot"))
            }]
        );
        assert_eq!(result.order_by_items, vec![]);
        assert_eq!(result.payload.get(), r#"{"a":1}"#);
    }

    #[test]
    pub fn query_result_deserializes_full_content() {
        const JSON: &str = r#"{"orderByItems":[{"item":1}], "groupByItems":[{"item":"yoot"}], "payload": {"a":1}}"#;
        let result: QueryResult = serde_json::from_str(JSON).unwrap();
        assert_eq!(
            result.group_by_items,
            vec![QueryClauseItem {
                item: Some(serde_json::json!("yoot"))
            }]
        );
        assert_eq!(
            result.order_by_items,
            vec![QueryClauseItem {
                item: Some(serde_json::json!(1))
            }]
        );
        assert_eq!(result.payload.get(), r#"{"a":1}"#);
    }

    #[test]
    pub fn query_result_can_be_created_from_raw_payload() {
        const JSON: &str = r#"{"a":1}"#;
        let result = QueryResult::from_payload(
            serde_json::value::RawValue::from_string(JSON.to_string()).unwrap(),
        );
        assert_eq!(result.group_by_items, vec![]);
        assert_eq!(result.order_by_items, vec![]);
        assert_eq!(result.payload.get(), r#"{"a":1}"#);
    }

    macro_rules! ordering_tests {
        (
            $(
                $name:ident {
                    $($left:tt, $right:tt => $expected:pat,)*
                }
            )+
        ) => {
            $(
                #[test]
                pub fn $name() {
                    $(
                        let left = serde_json::json!($left);
                        let right = serde_json::json!($right);
                        let left: QueryClauseItem = serde_json::from_value(left).unwrap();
                        let right: QueryClauseItem  = serde_json::from_value(right).unwrap();
                        let result = left.compare(&right);
                        assert!(matches!(result, $expected), "comparing {:?} and {:?}, expected: {}, but got {:?}", left, right, stringify!($expected), result);
                    )*
                }
            )+
        };
    }

    ordering_tests! {
        compare_numbers {
            {"item": 1}, {"item": 1} => Ok(Ordering::Equal),
            {"item": 1}, {"item": 2} => Ok(Ordering::Less),
            {"item": 2}, {"item": 1} => Ok(Ordering::Greater),
            {"item": 1.0}, {"item": 1.0} => Ok(Ordering::Equal),
            {"item": 1.0}, {"item": 1.1} => Ok(Ordering::Less),
            {"item": 1.1}, {"item": 1.0} => Ok(Ordering::Greater),
            {"item": -1}, {"item": -1} => Ok(Ordering::Equal),
            {"item": -1}, {"item": 1} => Ok(Ordering::Less),
            {"item": 1}, {"item": -1} => Ok(Ordering::Greater),
        }

        compare_bools {
            {"item": true}, {"item": false} => Ok(Ordering::Greater),
            {"item": false}, {"item": true} => Ok(Ordering::Less),
            {"item": true}, {"item": true} => Ok(Ordering::Equal),
            {"item": false}, {"item": false} => Ok(Ordering::Equal),
        }

        compare_strings {
            {"item": "aaa"}, {"item": "aab"} => Ok(Ordering::Less),
            {"item": "aab"}, {"item": "aaa"} => Ok(Ordering::Greater),
            {"item": "aaa"}, {"item": "aaa"} => Ok(Ordering::Equal),
        }

        compare_nulls_and_undefined {
            {}, {} => Ok(Ordering::Equal),
            {"item": null}, {"item": null} => Ok(Ordering::Equal),
            {}, {"item": null} => Ok(Ordering::Less),
        }

        compare_mixed_types {
            {}, {"item": null} => Ok(Ordering::Less),
            {"item": null}, {"item": true} => Ok(Ordering::Less),
            {"item": true}, {"item": 1} => Ok(Ordering::Less),
            {"item": 1}, {"item": "a"} => Ok(Ordering::Less),
        }

        cannot_compare_non_primitives {
            {"item": {"a": 1}}, {"item": {"a": 2}} => Err(_),
            {"item": [1, 2]}, {"item": [3, 4]} => Err(_),
            {"item": {"a": 1}}, {} => Err(_),
        }
    }

    #[test]
    pub fn compare_query_results_different() {
        let left = QueryResult {
            order_by_items: vec![
                QueryClauseItem {
                    item: Some(serde_json::json!(1)),
                },
                QueryClauseItem {
                    item: Some(serde_json::json!("zzzz")),
                },
            ],
            group_by_items: vec![],
            payload: serde_json::value::RawValue::from_string(r#"{"a":1}"#.to_string()).unwrap(),
        };
        let right = QueryResult {
            order_by_items: vec![
                QueryClauseItem {
                    item: Some(serde_json::json!(1)),
                },
                QueryClauseItem {
                    item: Some(serde_json::json!("yyyy")),
                },
            ],
            group_by_items: vec![],
            payload: serde_json::value::RawValue::from_string(r#"{"a":1}"#.to_string()).unwrap(),
        };
        assert_eq!(
            Ordering::Less,
            left.compare(&right, &[SortOrder::Ascending, SortOrder::Descending])
                .unwrap()
        );
    }

    #[test]
    pub fn compare_query_results_identical() {
        let left = QueryResult {
            order_by_items: vec![
                QueryClauseItem {
                    item: Some(serde_json::json!(1)),
                },
                QueryClauseItem {
                    item: Some(serde_json::json!("zzzz")),
                },
            ],
            group_by_items: vec![],
            payload: serde_json::value::RawValue::from_string(r#"{"a":1}"#.to_string()).unwrap(),
        };
        let right = QueryResult {
            order_by_items: vec![
                QueryClauseItem {
                    item: Some(serde_json::json!(1)),
                },
                QueryClauseItem {
                    item: Some(serde_json::json!("zzzz")),
                },
            ],
            group_by_items: vec![],
            payload: serde_json::value::RawValue::from_string(r#"{"a":1}"#.to_string()).unwrap(),
        };
        assert_eq!(
            Ordering::Equal,
            left.compare(&right, &[SortOrder::Ascending, SortOrder::Descending])
                .unwrap()
        );
    }

    #[test]
    pub fn compare_query_results_inconsistent() {
        let left = QueryResult {
            order_by_items: vec![QueryClauseItem {
                item: Some(serde_json::json!(1)),
            }],
            group_by_items: vec![],
            payload: serde_json::value::RawValue::from_string(r#"{"a":1}"#.to_string()).unwrap(),
        };
        let right = QueryResult {
            order_by_items: vec![
                QueryClauseItem {
                    item: Some(serde_json::json!(1)),
                },
                QueryClauseItem {
                    item: Some(serde_json::json!("zzzz")),
                },
            ],
            group_by_items: vec![],
            payload: serde_json::value::RawValue::from_string(r#"{"a":1}"#.to_string()).unwrap(),
        };
        let err = left
            .compare(&right, &[SortOrder::Ascending, SortOrder::Descending])
            .unwrap_err();
        assert_eq!(&ErrorKind::QueryPlanInvalid, err.kind());
    }
}
