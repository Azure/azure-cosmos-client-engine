use std::str::FromStr;

use serde::Deserialize;

use crate::{query::QueryClauseItem, ErrorKind};

#[derive(Clone, Debug)]
pub enum Aggregator {
    Count { count: u64 },
    Sum { sum: f64 },
    Average { sum: f64, count: u64 },
    Min { min: Option<QueryClauseItem> },
    Max { max: Option<QueryClauseItem> },
}

impl FromStr for Aggregator {
    type Err = crate::Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        // A match statement seems like the right thing to do, but it means forcing the string to lowercase first.
        // This allows us to do the comparison in a case-insensitive way without having to allocate a new string.
        if s.eq_ignore_ascii_case("count") {
            Ok(Aggregator::Count { count: 0 })
        } else if s.eq_ignore_ascii_case("sum") {
            Ok(Aggregator::Sum { sum: 0.0 })
        } else if s.eq_ignore_ascii_case("average") {
            Ok(Aggregator::Average { sum: 0.0, count: 0 })
        } else if s.eq_ignore_ascii_case("min") {
            Ok(Aggregator::Min { min: None })
        } else if s.eq_ignore_ascii_case("max") {
            Ok(Aggregator::Max { max: None })
        } else {
            Err(ErrorKind::UnsupportedQueryPlan.with_message(format!("unknown aggregator: {}", s)))
        }
    }
}

impl Aggregator {
    pub fn into_value(self) -> crate::Result<serde_json::Value> {
        fn make_number(value: f64) -> crate::Result<serde_json::Number> {
            serde_json::Number::from_f64(value).ok_or_else(|| {
                crate::ErrorKind::ArithmeticOverflow.with_message("aggregator has non-finite value")
            })
        }

        let value = match self {
            Aggregator::Count { count } => serde_json::Value::Number(count.into()),
            Aggregator::Sum { sum } => serde_json::Value::Number(make_number(sum)?),
            Aggregator::Average { sum, count } => {
                let avg = if count == 0 {
                    0.0
                } else {
                    sum / (count as f64)
                };
                serde_json::Value::Number(make_number(avg)?)
            }
            Aggregator::Min { min, .. } => {
                min.and_then(|c| c.item).unwrap_or(serde_json::Value::Null)
            }
            Aggregator::Max { max, .. } => {
                max.and_then(|c| c.item).unwrap_or(serde_json::Value::Null)
            }
        };
        Ok(value)
    }

    /// Aggregates the current value with the provided value, updating it in place.
    pub fn aggregate(&mut self, clause_item: &QueryClauseItem) -> crate::Result<()> {
        match self {
            Aggregator::Count { count } => {
                let value = require_non_null_value(clause_item, "count")?;
                let int_value = value.as_u64().ok_or_else(|| {
                    crate::ErrorKind::InvalidGatewayResponse
                        .with_message("count aggregator expects an integer value")
                })?;
                *count += int_value;
            }
            Aggregator::Sum { sum } => {
                let value = require_non_null_value(clause_item, "sum")?;
                let num_value = value.as_f64().ok_or_else(|| {
                    crate::ErrorKind::InvalidGatewayResponse
                        .with_message("sum aggregator expects a numeric value")
                })?;
                *sum += num_value;
            }
            Aggregator::Average { sum, count } => {
                let value = require_non_null_value(clause_item, "average")?;
                #[derive(Debug, Deserialize)]
                struct AverageItem {
                    sum: f64,
                    count: u64,
                }
                let item: AverageItem = serde_json::from_value(value.clone()).map_err(|e| {
                    crate::ErrorKind::InvalidGatewayResponse.with_message(format!(
                        "average aggregator expects object with 'sum' and 'count' properties: {}",
                        e
                    ))
                })?;
                *sum += item.sum;
                *count += item.count;
            }
            Aggregator::Min { min } => {
                if let Some(new) =
                    better_minmax_candidate(min, clause_item, std::cmp::Ordering::Less)?
                {
                    *min = Some(new);
                }
            }
            Aggregator::Max { max } => {
                if let Some(new) =
                    better_minmax_candidate(max, clause_item, std::cmp::Ordering::Greater)?
                {
                    *max = Some(new);
                }
            }
        }
        Ok(())
    }
}

/// Helper function to extract a non-null value from a QueryClauseItem.
fn require_non_null_value<'a>(
    clause_item: &'a QueryClauseItem,
    aggregator_name: &str,
) -> crate::Result<&'a serde_json::Value> {
    clause_item.item.as_ref().ok_or_else(|| {
        crate::ErrorKind::InvalidGatewayResponse.with_message(format!(
            "{} aggregator expects a non-null value",
            aggregator_name
        ))
    })
}

fn better_minmax_candidate(
    current: &Option<QueryClauseItem>,
    candidate: &QueryClauseItem,
    preferred_ordering: std::cmp::Ordering,
) -> crate::Result<Option<QueryClauseItem>> {
    let candidate_value = match &candidate.item {
        Some(serde_json::Value::Object(_)) => {
            #[derive(Debug, Deserialize)]
            struct MinMaxItem {
                #[serde(alias = "min")]
                #[serde(alias = "max")]
                value: serde_json::Value,
                count: u64,
            }

            let item: MinMaxItem = serde_json::from_value(candidate.item.clone().unwrap())
                .map_err(|e| {
                    crate::ErrorKind::InvalidGatewayResponse.with_message(format!(
                        "max aggregator expects object with 'max' and 'count' properties: {}",
                        e
                    ))
                })?;
            if item.count == 0 {
                // Ignore aggregation if count is zero
                return Ok(None);
            }
            QueryClauseItem::from_value(item.value)
        }
        _ => candidate.clone(),
    };

    Ok(match current {
        None => Some(candidate_value),
        Some(existing) if candidate_value.compare(existing)? == preferred_ordering => {
            Some(candidate_value)
        }
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn count() -> crate::Result<()> {
        let mut aggregator = Aggregator::Count { count: 0 };

        aggregator.aggregate(&QueryClauseItem::from_value(json!(5)))?;
        aggregator.aggregate(&QueryClauseItem::from_value(json!(3)))?;
        aggregator.aggregate(&QueryClauseItem::from_value(json!(7)))?;

        let result = aggregator.into_value()?;
        assert_eq!(result, json!(15));

        Ok(())
    }

    #[test]
    fn count_zero_values() -> crate::Result<()> {
        let mut aggregator = Aggregator::Count { count: 0 };

        aggregator.aggregate(&QueryClauseItem::from_value(json!(0)))?;
        aggregator.aggregate(&QueryClauseItem::from_value(json!(0)))?;

        let result = aggregator.into_value()?;
        assert_eq!(result, json!(0));

        Ok(())
    }

    #[test]
    fn count_empty() -> crate::Result<()> {
        let aggregator = Aggregator::Count { count: 0 };

        let result = aggregator.into_value()?;
        assert_eq!(result, json!(0));

        Ok(())
    }

    #[test]
    fn sum() -> crate::Result<()> {
        let mut aggregator = Aggregator::Sum { sum: 0.0 };

        aggregator.aggregate(&QueryClauseItem::from_value(json!(10.5)))?;
        aggregator.aggregate(&QueryClauseItem::from_value(json!(20)))?;
        aggregator.aggregate(&QueryClauseItem::from_value(json!(-5.5)))?;

        let result = aggregator.into_value()?;
        assert_eq!(result, json!(25.0));

        Ok(())
    }

    #[test]
    fn sum_empty() -> crate::Result<()> {
        let aggregator = Aggregator::Sum { sum: 0.0 };

        let result = aggregator.into_value()?;
        assert_eq!(result, json!(0.0));

        Ok(())
    }

    #[test]
    fn average() -> crate::Result<()> {
        let mut aggregator = Aggregator::Average { sum: 0.0, count: 0 };

        aggregator.aggregate(&QueryClauseItem::from_value(
            json!({"sum": 10.0, "count": 2}),
        ))?;
        aggregator.aggregate(&QueryClauseItem::from_value(
            json!({"sum": 15.0, "count": 3}),
        ))?;
        aggregator.aggregate(&QueryClauseItem::from_value(
            json!({"sum": 5.0, "count": 1}),
        ))?;

        let result = aggregator.into_value()?;
        assert_eq!(result, json!(5.0));

        Ok(())
    }

    #[test]
    fn average_empty() -> crate::Result<()> {
        let aggregator = Aggregator::Average { sum: 0.0, count: 0 };

        // Returns 0.0 when count is 0 to avoid division by zero
        let result = aggregator.into_value()?;
        assert_eq!(result, json!(0.0));

        Ok(())
    }

    #[test]
    fn min_with_objects() -> crate::Result<()> {
        let mut aggregator = Aggregator::Min { min: None };

        aggregator.aggregate(&QueryClauseItem::from_value(json!({"min": 10, "count": 1})))?;
        aggregator.aggregate(&QueryClauseItem::from_value(json!({"min": 5, "count": 2})))?;
        aggregator.aggregate(&QueryClauseItem::from_value(json!({"min": 15, "count": 1})))?;

        let result = aggregator.into_value()?;
        assert_eq!(result, json!(5));

        Ok(())
    }

    #[test]
    fn min_with_direct_values() -> crate::Result<()> {
        let mut aggregator = Aggregator::Min { min: None };

        aggregator.aggregate(&QueryClauseItem::from_value(json!(10)))?;
        aggregator.aggregate(&QueryClauseItem::from_value(json!(5)))?;
        aggregator.aggregate(&QueryClauseItem::from_value(json!(15)))?;

        let result = aggregator.into_value()?;
        assert_eq!(result, json!(5));

        Ok(())
    }

    #[test]
    fn min_ignore_zero_count() -> crate::Result<()> {
        let mut aggregator = Aggregator::Min { min: None };

        aggregator.aggregate(&QueryClauseItem::from_value(json!({"min": 10, "count": 1})))?;
        // Zero count values are ignored because they come from empty partitions
        aggregator.aggregate(&QueryClauseItem::from_value(json!({"min": 1, "count": 0})))?;

        let result = aggregator.into_value()?;
        assert_eq!(result, json!(10));

        Ok(())
    }

    #[test]
    fn min_empty() -> crate::Result<()> {
        let aggregator = Aggregator::Min { min: None };

        let result = aggregator.into_value()?;
        assert_eq!(result, json!(null));

        Ok(())
    }

    #[test]
    fn max_with_objects() -> crate::Result<()> {
        let mut aggregator = Aggregator::Max { max: None };

        aggregator.aggregate(&QueryClauseItem::from_value(json!({"max": 10, "count": 1})))?;
        aggregator.aggregate(&QueryClauseItem::from_value(json!({"max": 5, "count": 2})))?;
        aggregator.aggregate(&QueryClauseItem::from_value(json!({"max": 15, "count": 1})))?;

        let result = aggregator.into_value()?;
        assert_eq!(result, json!(15));

        Ok(())
    }

    #[test]
    fn max_with_direct_values() -> crate::Result<()> {
        let mut aggregator = Aggregator::Max { max: None };

        aggregator.aggregate(&QueryClauseItem::from_value(json!(10)))?;
        aggregator.aggregate(&QueryClauseItem::from_value(json!(5)))?;
        aggregator.aggregate(&QueryClauseItem::from_value(json!(15)))?;

        let result = aggregator.into_value()?;
        assert_eq!(result, json!(15));

        Ok(())
    }

    #[test]
    fn max_ignore_zero_count() -> crate::Result<()> {
        let mut aggregator = Aggregator::Max { max: None };

        aggregator.aggregate(&QueryClauseItem::from_value(json!({"max": 10, "count": 1})))?;
        // Zero count values are ignored because they come from empty partitions
        aggregator.aggregate(&QueryClauseItem::from_value(
            json!({"max": 100, "count": 0}),
        ))?;

        let result = aggregator.into_value()?;
        assert_eq!(result, json!(10));

        Ok(())
    }

    #[test]
    fn max_empty() -> crate::Result<()> {
        let aggregator = Aggregator::Max { max: None };

        let result = aggregator.into_value()?;
        assert_eq!(result, json!(null));

        Ok(())
    }

    #[test]
    fn min_max_with_strings() -> crate::Result<()> {
        let mut min_aggregator = Aggregator::Min { min: None };
        let mut max_aggregator = Aggregator::Max { max: None };

        min_aggregator.aggregate(&QueryClauseItem::from_value(json!("banana")))?;
        min_aggregator.aggregate(&QueryClauseItem::from_value(json!("apple")))?;
        min_aggregator.aggregate(&QueryClauseItem::from_value(json!("cherry")))?;

        max_aggregator.aggregate(&QueryClauseItem::from_value(json!("banana")))?;
        max_aggregator.aggregate(&QueryClauseItem::from_value(json!("apple")))?;
        max_aggregator.aggregate(&QueryClauseItem::from_value(json!("cherry")))?;

        let min_result = min_aggregator.into_value()?;
        let max_result = max_aggregator.into_value()?;
        assert_eq!(min_result, json!("apple"));
        assert_eq!(max_result, json!("cherry"));

        Ok(())
    }
}
