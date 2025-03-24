use serde::{Deserialize, Serialize};


#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OrderId {
    #[serde(rename = "orderId")]
    pub orderId: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OrderPrimId {
    #[serde(rename = "orderPrimId")]
    pub orderPrimId: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductId {
    #[serde(rename = "productId")]
    pub productId: String,
}

pub fn to_order_id(id: String) -> OrderId {
    OrderId {
        orderId: id,
    }
}

pub fn to_order_prim_id(id: i64) -> OrderPrimId {
    OrderPrimId {
        orderPrimId: id,
    }
}

pub fn to_product_id(id: String) -> ProductId {
    ProductId {
        productId: id,
    }
}