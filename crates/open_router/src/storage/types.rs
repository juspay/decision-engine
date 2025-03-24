use diesel::{
    backend::Backend,
    deserialize::{self, FromSql},
    serialize::ToSql,
    sql_types, AsExpression, Queryable,
};
use masking::{PeekInterface, Secret, StrongSecret};

// use super::schema;

// #[derive(Debug, Identifiable, Queryable)]
// #[diesel(table_name = schema::merchant)]
// pub(super) struct MerchantInner {
//     id: i32,
//     merchant_id: String,
//     enc_key: Encrypted,
//     created_at: time::PrimitiveDateTime,
// }

#[derive(Debug, Clone)]
pub struct Merchant {
    pub merchant_id: String,
    pub enc_key: Secret<Vec<u8>>,
    pub created_at: time::PrimitiveDateTime,
}

// #[derive(Debug, Insertable)]
// #[diesel(table_name = schema::merchant)]
// pub(super) struct MerchantNewInner<'a> {
//     merchant_id: &'a str,
//     enc_key: Encrypted,
// }

#[derive(Debug)]
pub struct MerchantNew<'a> {
    pub merchant_id: &'a str,
    pub enc_key: Secret<Vec<u8>>,
}

// #[derive(Debug, Identifiable, Queryable)]
// #[diesel(table_name = schema::open_router)]
// pub(super) struct OpenRouterInner {
//     id: i32,
//     open_router_id: Secret<String>,
//     merchant_id: String,
//     customer_id: String,
//     enc_data: Encrypted,
//     created_at: time::PrimitiveDateTime,
//     hash_id: String,
//     ttl: Option<time::PrimitiveDateTime>,
// }

// impl From<OpenRouterInner> for OpenRouter {
//     fn from(value: OpenRouterInner) -> Self {
//         Self {
//             open_router_id: value.open_router_id,
//             merchant_id: value.merchant_id,
//             customer_id: value.customer_id,
//             data: value.enc_data.into(),
//             created_at: value.created_at,
//             hash_id: value.hash_id,
//             ttl: value.ttl,
//         }
//     }
// }

#[derive(Debug)]
pub struct OpenRouter {
    pub open_router_id: Secret<String>,
    pub merchant_id: String,
    pub customer_id: String,
    pub data: Encryptable,
    pub created_at: time::PrimitiveDateTime,
    pub hash_id: String,
    pub ttl: Option<time::PrimitiveDateTime>,
}

#[derive(Debug)]
pub enum Encryptable {
    Encrypted(Secret<Vec<u8>>),
    Decrypted(StrongSecret<Vec<u8>>),
}

impl Encryptable {
    pub fn get_encrypted_inner_value(&self) -> Option<Secret<Vec<u8>>> {
        match self {
            Self::Encrypted(secret) => Some(secret.clone()),
            Self::Decrypted(_) => None,
        }
    }

    pub fn get_decrypted_inner_value(&self) -> Option<StrongSecret<Vec<u8>>> {
        match self {
            Self::Encrypted(_) => None,
            Self::Decrypted(secret) => Some(secret.clone()),
        }
    }

    pub fn from_decrypted_data(decrypted_data: StrongSecret<Vec<u8>>) -> Self {
        Self::Decrypted(decrypted_data)
    }
}

impl From<Encrypted> for Encryptable {
    fn from(value: Encrypted) -> Self {
        Self::Encrypted(value.into_inner())
    }
}


#[derive(Debug, serde::Serialize, serde::Deserialize, Eq, PartialEq, Clone)]
pub struct CardNumber(StrongSecret<String>);

impl CardNumber {
    pub fn into_bytes(self) -> Vec<u8> {
        self.0.peek().clone().into_bytes()
    }
}

impl std::ops::Deref for CardNumber {
    type Target = StrongSecret<String>;

    fn deref(&self) -> &StrongSecret<String> {
        &self.0
    }
}

///
/// Type representing data stored in ecrypted state in the database
///
#[derive(Debug, Clone, AsExpression)]
#[diesel(sql_type = diesel::sql_types::Binary)]
#[repr(transparent)]
pub struct Encrypted {
    inner: Secret<Vec<u8>>,
}

impl Encrypted {
    pub fn new(item: Secret<Vec<u8>>) -> Self {
        Self { inner: item }
    }

    #[inline]
    pub fn into_inner(self) -> Secret<Vec<u8>> {
        self.inner
    }

    #[inline]
    pub fn get_inner(&self) -> &Secret<Vec<u8>> {
        &self.inner
    }
}

impl From<Vec<u8>> for Encrypted {
    fn from(value: Vec<u8>) -> Self {
        Self {
            inner: value.into(),
        }
    }
}

impl From<Secret<Vec<u8>>> for Encrypted {
    fn from(value: Secret<Vec<u8>>) -> Self {
        Self::new(value)
    }
}

impl<DB> FromSql<sql_types::Binary, DB> for Encrypted
where
    DB: Backend,
    Secret<Vec<u8>>: FromSql<sql_types::Binary, DB>,
{
    fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
        <Secret<Vec<u8>>>::from_sql(bytes).map(Self::new)
    }
}

impl<DB> ToSql<sql_types::Binary, DB> for Encrypted
where
    DB: Backend,
    Secret<Vec<u8>>: ToSql<sql_types::Binary, DB>,
{
    fn to_sql<'b>(
        &'b self,
        out: &mut diesel::serialize::Output<'b, '_, DB>,
    ) -> diesel::serialize::Result {
        self.get_inner().to_sql(out)
    }
}

impl<DB> Queryable<sql_types::Binary, DB> for Encrypted
where
    DB: Backend,
    Secret<Vec<u8>>: FromSql<sql_types::Binary, DB>,
{
    type Row = Secret<Vec<u8>>;
    fn build(row: Self::Row) -> deserialize::Result<Self> {
        Ok(Self { inner: row })
    }
}

