//! Binding a request to the merchant it may act on.
//!
//! Every request that acts on one merchant resolves an [`AuthorizedMerchant`] before it touches
//! that merchant's data, and storage that holds per-merchant rows takes one instead of a bare id.
//! Only this module constructs it, so a handler that skips authorization has nothing to hand to
//! storage.
//!
//! - A per-merchant session (JWT or api key) is authorized for its own merchant and that merchant's
//!   sub-merchants. A request that names any other merchant is refused with `403`.
//! - A service credential (the admin secret, or the `api_key_auth_enabled = false` compat lane)
//!   carries no session and acts on behalf of whichever merchant the request names. This is how
//!   Hyperswitch calls Decision Engine.
//!
//! # Adding an endpoint
//!
//! 1. Register it in the route group for its caller, in `app.rs`: `merchant_routes()` for anything
//!    acting on one merchant, `admin_routes()` for deployment-level calls only the admin secret may
//!    make, `public_routes()` for routes that authenticate themselves (sign-in, signed webhooks).
//!    Register each method with `metered!(post, module::handler)` (`crate::middleware::metered`),
//!    which records its request metrics under the handler's name; the handler records none itself.
//! 2. Take the merchant from the request with an extractor — handlers never call [`authorize`]:
//!    - Named in the path: spell the parameter `:merchant_id` ([`MERCHANT_PATH_PARAM`]) and take
//!      [`MerchantFromPath`](crate::custom_extractors::MerchantFromPath).
//!    - Named in the body: `#[derive(MerchantScoped)]` on the request type, `#[merchant]` on every
//!      field naming a merchant (nested request types included), and take
//!      [`ScopedJson<T>`](crate::custom_extractors::ScopedJson). A handler with its own error
//!      format takes `Result<ScopedJson<T>, ScopedJsonRejection>` and answers each rejection.
//!    - Named by neither, only by an object id: the session's merchant ([`session_merchant`]),
//!      or for a service caller the object's owner, as api-key revoke does.
//! 3. Pass the [`AuthorizedMerchant`] to storage. A store holding per-merchant rows takes one and
//!    filters every query by it, so another merchant's object reads as not found.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use super::AuthContext;

pub use merchant_scoped_derive::MerchantScoped;

/// The path parameter that names the merchant a route acts on. Every merchant-addressed route
/// spells it this way, so the scope guard and [`crate::custom_extractors::MerchantFromPath`] find
/// it without a per-route list.
pub const MERCHANT_PATH_PARAM: &str = "merchant_id";

/// A request that names the merchant(s) it acts on.
///
/// Derived on each request type that carries a merchant in its body, with `#[merchant]` on every
/// field naming one — a merchant field left unmarked is one a session can reach across to. A
/// marked field is itself `MerchantScoped`: an id ([`String`], [`MerchantId`]), an [`Option`] of
/// one, or a nested type that derives it.
///
/// [`MerchantId`]: crate::types::merchant::id::MerchantId
pub trait MerchantScoped {
    fn merchant_ids(&self) -> Vec<&str>;
}

impl MerchantScoped for str {
    fn merchant_ids(&self) -> Vec<&str> {
        vec![self]
    }
}

impl MerchantScoped for String {
    fn merchant_ids(&self) -> Vec<&str> {
        vec![self.as_str()]
    }
}

impl MerchantScoped for crate::types::merchant::id::MerchantId {
    fn merchant_ids(&self) -> Vec<&str> {
        vec![self.0.as_str()]
    }
}

/// An optional field names a merchant only when present.
impl<T: MerchantScoped> MerchantScoped for Option<T> {
    fn merchant_ids(&self) -> Vec<&str> {
        self.as_ref().map(T::merchant_ids).unwrap_or_default()
    }
}

/// A merchant the current request is authorized to act on. Requests construct one only through
/// [`authorize`], [`authorize_merchant_id`] and [`session_merchant`]; work Decision Engine starts
/// itself uses [`AuthorizedMerchant::internal`], and a signed callback
/// [`AuthorizedMerchant::signed_callback`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizedMerchant(String);

impl AuthorizedMerchant {
    /// The merchant that work Decision Engine starts itself acts on — a scheduler, a poller, a
    /// background refresh — whose id comes from Decision Engine's own storage. Never call this with
    /// an id taken from a request: requests are authorized with [`authorize`] or
    /// [`authorize_merchant_id`].
    pub fn internal(merchant_id: impl Into<String>) -> Self {
        Self(merchant_id.into())
    }

    /// The merchant an unauthenticated, signed callback (a connector settlement webhook) names in
    /// its path. The signature — checked against that merchant's own secret — is what authorizes it:
    /// use this to load the merchant's verification secret, and act on the callback only once the
    /// signature verifies.
    pub fn signed_callback(merchant_id: impl Into<String>) -> Self {
        Self(merchant_id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for AuthorizedMerchant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Why a request could not be authorized for a merchant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MerchantScopeViolation {
    /// A per-merchant session named a merchant other than its own.
    ForeignMerchant,
    /// The request names its merchant in more than one place, and they disagree.
    ConflictingMerchants,
    /// A service credential sent a request that names no merchant.
    NoMerchant,
}

impl MerchantScopeViolation {
    pub fn status(self) -> StatusCode {
        match self {
            Self::ForeignMerchant => StatusCode::FORBIDDEN,
            Self::ConflictingMerchants | Self::NoMerchant => StatusCode::BAD_REQUEST,
        }
    }

    pub fn message(self) -> &'static str {
        match self {
            Self::ForeignMerchant => "This session may only act on its own merchant",
            Self::ConflictingMerchants => "The request names more than one merchant",
            Self::NoMerchant => "The request does not name a merchant",
        }
    }
}

impl std::fmt::Display for MerchantScopeViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message())
    }
}

impl IntoResponse for MerchantScopeViolation {
    fn into_response(self) -> Response {
        crate::error::text_error_response(self.status(), self.message())
    }
}

/// Resolve the merchant a body-addressed request may act on: the one it names, or else the
/// session's own.
///
/// Every merchant the request names must be the same one. A session may act on its own merchant and
/// on that merchant's sub-merchants; anything else is refused. A service credential acts on whatever
/// the request names.
pub fn authorize(
    session: Option<&AuthContext>,
    request: &(impl MerchantScoped + ?Sized),
) -> impl std::future::Future<Output = Result<AuthorizedMerchant, MerchantScopeViolation>> + Send + 'static
{
    // Copied out before the lookup, so the future borrows neither the request nor the session.
    let named: Vec<String> = request
        .merchant_ids()
        .into_iter()
        .map(str::to_owned)
        .collect();
    let session_merchant = session.map(|session| session.merchant_id.clone());
    async move {
        let named: Vec<&str> = named.iter().map(String::as_str).collect();
        let Some(target) = named.first().copied().or(session_merchant.as_deref()) else {
            return Err(MerchantScopeViolation::NoMerchant);
        };
        // The parent is read only when a session names a merchant other than its own, so a request
        // on the session's own merchant — nearly every one — costs no lookup.
        let parent = match session_merchant.as_deref() {
            Some(own) if own != target => parent_of(target).await,
            _ => None,
        };
        resolve(
            session_merchant.as_deref(),
            &named,
            target,
            parent.as_deref(),
        )
    }
}

/// The decision [`authorize`] makes, given the parent of the merchant the request is for.
fn resolve(
    session_merchant: Option<&str>,
    named: &[&str],
    target: &str,
    parent: Option<&str>,
) -> Result<AuthorizedMerchant, MerchantScopeViolation> {
    if let Some(own) = session_merchant {
        if target != own && parent != Some(own) {
            return Err(MerchantScopeViolation::ForeignMerchant);
        }
    }
    if named.iter().any(|merchant_id| *merchant_id != target) {
        return Err(match session_merchant {
            Some(_) => MerchantScopeViolation::ForeignMerchant,
            None => MerchantScopeViolation::ConflictingMerchants,
        });
    }
    Ok(AuthorizedMerchant(target.to_string()))
}

/// The merchant `merchant_id` is a sub-merchant of. Served from the cached account that routing
/// reads anyway, and never stale in a way that matters: a sub-merchant's parent is set once, when it
/// is created.
async fn parent_of(merchant_id: &str) -> Option<String> {
    // Without storage — a unit test — nothing has been created as a sub-merchant.
    crate::app::APP_STATE.get()?;
    crate::types::merchant::merchant_account::load_merchant_by_merchant_id(merchant_id.to_string())
        .await
        .and_then(|account| account.parentMerchantId)
}

/// The merchant that holds the API keys for `merchant`: its parent when it is a sub-merchant, which
/// has none of its own, and otherwise itself. Authorization for a sub-merchant carries this one step
/// up for managing keys only — never for acting on the parent's routing.
pub async fn key_holder(merchant: &AuthorizedMerchant) -> AuthorizedMerchant {
    match parent_of(merchant.as_str()).await {
        Some(parent) => AuthorizedMerchant(parent),
        None => merchant.clone(),
    }
}

/// The merchant a session is authorized for, for a route that names no merchant of its own (such as
/// revoking an api key by id). `None` for a service credential, which has no merchant of its own.
pub fn session_merchant(session: Option<&AuthContext>) -> Option<AuthorizedMerchant> {
    session.map(|session| AuthorizedMerchant(session.merchant_id.clone()))
}

/// Resolve a request that names its merchant once, as a path parameter does.
pub fn authorize_merchant_id(
    session: Option<&AuthContext>,
    merchant_id: &str,
) -> impl std::future::Future<Output = Result<AuthorizedMerchant, MerchantScopeViolation>> + Send + 'static
{
    authorize(session, merchant_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `resolve` as `authorize` calls it, with `sub_a1` and `sub_a2` the sub-merchants of `a`.
    fn authorized(
        session: Option<&str>,
        names: Vec<&'static str>,
    ) -> Result<String, MerchantScopeViolation> {
        let Some(target) = names.first().copied().or(session) else {
            return Err(MerchantScopeViolation::NoMerchant);
        };
        let parent = target.starts_with("sub_a").then_some("a");
        resolve(session, &names, target, parent).map(|merchant| merchant.to_string())
    }

    #[test]
    fn a_session_reaches_its_sub_merchants_and_nothing_else() {
        assert_eq!(authorized(Some("a"), vec!["sub_a1"]), Ok("sub_a1".into()));
        assert_eq!(
            authorized(Some("b"), vec!["sub_a1"]),
            Err(MerchantScopeViolation::ForeignMerchant)
        );
        // A session on a sub-merchant does not reach its parent or its siblings.
        assert_eq!(
            authorized(Some("sub_a1"), vec!["a"]),
            Err(MerchantScopeViolation::ForeignMerchant)
        );
        assert_eq!(
            authorized(Some("sub_a1"), vec!["sub_a2"]),
            Err(MerchantScopeViolation::ForeignMerchant)
        );
    }

    #[test]
    fn a_parent_and_its_sub_merchant_named_together_still_conflict() {
        assert_eq!(
            authorized(None, vec!["sub_a1", "a"]),
            Err(MerchantScopeViolation::ConflictingMerchants)
        );
        assert_eq!(
            authorized(Some("a"), vec!["a", "sub_a1"]),
            Err(MerchantScopeViolation::ForeignMerchant)
        );
    }

    #[test]
    fn a_session_is_authorized_for_its_own_merchant_only() {
        assert_eq!(authorized(Some("a"), vec!["a"]), Ok("a".to_string()));
        assert_eq!(
            authorized(Some("a"), vec!["b"]),
            Err(MerchantScopeViolation::ForeignMerchant)
        );
        assert_eq!(
            authorized(Some("a"), vec!["a", "b"]),
            Err(MerchantScopeViolation::ForeignMerchant)
        );
    }

    #[test]
    fn a_session_request_that_names_no_merchant_acts_on_the_sessions() {
        assert_eq!(authorized(Some("a"), vec![]), Ok("a".to_string()));
    }

    #[test]
    fn a_service_credential_acts_on_the_merchant_the_request_names() {
        assert_eq!(authorized(None, vec!["b", "b"]), Ok("b".to_string()));
        assert_eq!(
            authorized(None, vec!["b", "c"]),
            Err(MerchantScopeViolation::ConflictingMerchants)
        );
        assert_eq!(
            authorized(None, vec![]),
            Err(MerchantScopeViolation::NoMerchant)
        );
    }

    #[derive(MerchantScoped)]
    struct Inner {
        #[merchant]
        merchant_id: crate::types::merchant::id::MerchantId,
    }

    #[derive(MerchantScoped)]
    struct Request {
        #[merchant]
        created_by: String,
        #[allow(dead_code)]
        name: String,
        #[merchant]
        dynamic: Option<Inner>,
        #[merchant]
        nested: Inner,
    }

    fn request(dynamic: Option<&str>) -> Request {
        let inner = |id: &str| Inner {
            merchant_id: crate::types::merchant::id::MerchantId(id.to_string()),
        };
        Request {
            created_by: "a".to_string(),
            name: "not a merchant".to_string(),
            dynamic: dynamic.map(inner),
            nested: inner("c"),
        }
    }

    #[test]
    fn the_derive_lists_every_marked_field_in_order() {
        assert_eq!(request(Some("b")).merchant_ids(), vec!["a", "b", "c"]);
    }

    #[test]
    fn an_absent_optional_field_names_no_merchant() {
        assert_eq!(request(None).merchant_ids(), vec!["a", "c"]);
    }

    #[test]
    fn a_derived_request_is_authorized_on_all_its_merchants() {
        let request = request(None);
        let named = request.merchant_ids();
        assert_eq!(
            resolve(Some("a"), &named, "a", None),
            Err(MerchantScopeViolation::ForeignMerchant)
        );
    }

    #[test]
    fn only_a_foreign_merchant_is_forbidden() {
        assert_eq!(
            MerchantScopeViolation::ForeignMerchant.status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            MerchantScopeViolation::ConflictingMerchants.status(),
            StatusCode::BAD_REQUEST
        );
    }
}
