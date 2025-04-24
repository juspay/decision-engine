/// Characters to use for generating NanoID
pub(crate) const ALPHABETS: [char; 62] = [
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i',
    'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z', 'A', 'B',
    'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U',
    'V', 'W', 'X', 'Y', 'Z',
];

/// Number of characters in a generated ID
pub const ID_LENGTH: usize = 20;

/// Header key for tenant ID
pub const X_TENANT_ID: &str = "x-tenant-id";
/// Header key for request ID
pub const X_REQUEST_ID: &str = "x-request-id";
/// Header key for udf order id
pub const UDF_ORDER_ID: &str = "x-order-id";
/// Header key for udf customer id
pub const UDF_CUSTOMER_ID: &str = "customer_id";
/// Header key for udf transaction UUID
pub const UDF_TXN_UUID: &str = "x-txn-uuid";
/// Header key for global request ID
pub const X_GLOBAL_REQUEST_ID: &str = "x-global-request-id";

pub const EULER_REQUEST_ID: &str = "euler-request-id";

pub const X_CELL_SELECTOR: &str = "x-cell-selector";

pub const X_SESSION_ID: &str = "x-session-id";

pub const X_ART_RECORDING: &str = "x-art-recording";
/// Header Constants
pub mod headers {
    pub const CONTENT_TYPE: &str = "Content-Type";
    pub const AUTHORIZATION: &str = "Authorization";
}
