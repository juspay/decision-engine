//! Storage for merchant API keys, scoped to the merchant that owns them.
//!
//! Every query is filtered by the [`AuthorizedMerchant`]'s id, so another merchant's key is simply
//! not found. Authentication looks keys up by hash instead, before any merchant is known; that
//! lookup lives in the authenticate middleware.

use crate::generics::Repository;
use crate::storage::backend::schema::merchant_api_keys::dsl;

use diesel::{associations::HasTable, BoolExpressionMethods, ExpressionMethods};
use error_stack::Report;

use crate::auth::AuthorizedMerchant;
use crate::generics::{self, MeshError};
use crate::storage::types::{MerchantApiKey, MerchantApiKeyNew, MerchantApiKeyRevoke};
use crate::storage::Storage;

type StoreResult<T> = Result<T, Report<MeshError>>;

/// Store a new key as the merchant's: `merchant_id` is set from `merchant`, whatever the row carried.
pub async fn insert(
    db: &Storage,
    merchant: &AuthorizedMerchant,
    mut key: MerchantApiKeyNew,
) -> StoreResult<()> {
    key.merchant_id = merchant.as_str().to_string();
    generics::generic_insert(db, key).await.map(|_| ())
}

/// Every key the merchant has, active or revoked.
pub async fn list(db: &Storage, merchant: &AuthorizedMerchant) -> StoreResult<Vec<MerchantApiKey>> {
    MerchantApiKey::find_all(db, dsl::merchant_id.eq(merchant.as_str().to_string()))
        .await
        .map_err(Report::new)
}

/// The merchant's key with this id.
pub async fn find(
    db: &Storage,
    merchant: &AuthorizedMerchant,
    key_id: &str,
) -> StoreResult<Option<MerchantApiKey>> {
    MerchantApiKey::find_optional(
        db,
        dsl::key_id
            .eq(key_id.to_string())
            .and(dsl::merchant_id.eq(merchant.as_str().to_string())),
    )
    .await
    .map_err(Report::new)
}

/// Revoke the merchant's key. Revoking one the merchant does not own changes nothing.
pub async fn revoke(db: &Storage, merchant: &AuthorizedMerchant, key_id: &str) -> StoreResult<()> {
    let conn = db.get_conn().await.map_err(Report::new)?;
    generics::generic_update::<<MerchantApiKey as HasTable>::Table, _, _>(
        &conn,
        dsl::key_id
            .eq(key_id.to_string())
            .and(dsl::merchant_id.eq(merchant.as_str().to_string())),
        MerchantApiKeyRevoke {
            #[cfg(feature = "mysql")]
            is_active: 0,
            #[cfg(feature = "postgres")]
            is_active: false,
        },
    )
    .await
    .map(|_| ())
}

/// The merchant that owns a key, for a service caller revoking by key id alone. A session is
/// authorized for its own merchant and never needs this.
pub async fn owner_of(db: &Storage, key_id: &str) -> StoreResult<Option<String>> {
    MerchantApiKey::find_optional(db, dsl::key_id.eq(key_id.to_string()))
        .await
        .map(|key| key.map(|key| key.merchant_id))
        .map_err(Report::new)
}
