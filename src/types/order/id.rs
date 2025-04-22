use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OrderId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OrderPrimId(i64);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductId(pub String);

pub fn to_order_id(id: String) -> OrderId {
    OrderId(id)
}

pub fn to_order_prim_id(id: i64) -> OrderPrimId {
    OrderPrimId(id)
}

pub fn to_product_id(id: String) -> ProductId {
    ProductId(id)
}
