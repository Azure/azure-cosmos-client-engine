pub trait QueryValue: Ord {}

pub trait QueryPayload {
    type Value: QueryValue;

    fn iter_order_by_values(&self) -> impl Iterator<Item = Self::Value>;
}
