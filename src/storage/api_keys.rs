//! Storage for merchant API keys, scoped to the merchant that owns them.
//!
//! Every query is filtered by the [`AuthorizedMerchant`]'s id, so another merchant's key is simply
//! not found. Authentication looks keys up by hash instead, before any merchant is known; that
//! lookup lives in the authenticate middleware.

use crate::generics::Repository;
use crate::storage::backend::schema::{
    api_key_merchants::dsl as grants, merchant_account::dsl as merchants, merchant_api_keys::dsl,
};

use async_bb8_diesel::AsyncConnection;
use diesel::{BoolExpressionMethods, Connection, ExpressionMethods, QueryDsl, RunQueryDsl};
use error_stack::{Report, ResultExt};

use crate::auth::AuthorizedMerchant;
use crate::generics::MeshError;
use crate::storage::types::{ApiKeyMerchant, MerchantApiKey, MerchantApiKeyNew};
use crate::storage::Storage;

type StoreResult<T> = Result<T, Report<MeshError>>;

/// Store a new single-merchant key and its owner grant atomically. The request cannot add grants.
pub async fn insert(
    db: &Storage,
    merchant: &AuthorizedMerchant,
    mut key: MerchantApiKeyNew,
) -> StoreResult<()> {
    key.merchant_id = merchant.as_str().to_string();
    let conn = db.get_conn().await.map_err(Report::new)?;
    conn.run(move |conn| {
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            let exists = diesel::select(diesel::dsl::exists(
                merchants::merchant_account.filter(merchants::merchant_id.eq(&key.merchant_id)),
            ))
            .get_result::<bool>(conn)?;
            #[cfg(feature = "mysql")]
            let active = key.is_active != 0;
            #[cfg(feature = "postgres")]
            let active = key.is_active;
            let grant = ApiKeyMerchant {
                key_id: key.key_id.clone(),
                merchant_id: key.merchant_id.clone(),
                status: if active && exists {
                    "active"
                } else {
                    "revoked"
                }
                .to_string(),
                permissions: None,
                authz_version: 1,
                created_at: key.created_at,
            };
            diesel::insert_into(dsl::merchant_api_keys)
                .values(key)
                .execute(conn)?;
            diesel::insert_into(grants::api_key_merchants)
                .values(grant)
                .execute(conn)?;
            Ok(())
        })
    })
    .await
    .change_context(MeshError::Others)
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
    let merchant_id = merchant.as_str().to_string();
    let key_id = key_id.to_string();
    conn.run(move |conn| {
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            let changed = diesel::update(
                dsl::merchant_api_keys.filter(
                    dsl::key_id
                        .eq(&key_id)
                        .and(dsl::merchant_id.eq(&merchant_id)),
                ),
            )
            .set((
                #[cfg(feature = "mysql")]
                dsl::is_active.eq(0_i8),
                #[cfg(feature = "postgres")]
                dsl::is_active.eq(false),
                dsl::authz_version.eq(dsl::authz_version + 1),
            ))
            .execute(conn)?;
            if changed == 0 {
                return Err(diesel::result::Error::NotFound);
            }
            diesel::update(grants::api_key_merchants.filter(grants::key_id.eq(&key_id)))
                .set((
                    grants::status.eq("revoked"),
                    grants::authz_version.eq(grants::authz_version + 1),
                ))
                .execute(conn)?;
            Ok(())
        })
    })
    .await
    .change_context(MeshError::Others)
}

/// The merchant that owns a key, for a service caller revoking by key id alone. A session is
/// authorized for its own merchant and never needs this.
pub async fn owner_of(db: &Storage, key_id: &str) -> StoreResult<Option<String>> {
    MerchantApiKey::find_optional(db, dsl::key_id.eq(key_id.to_string()))
        .await
        .map(|key| key.map(|key| key.merchant_id))
        .map_err(Report::new)
}
