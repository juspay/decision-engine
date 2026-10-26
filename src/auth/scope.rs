//! Binding a request to the merchant it may act on.
//!
//! Every request that acts on one merchant resolves an [`AuthorizedMerchant`] before it touches
//! that merchant's data, and storage that holds per-merchant rows takes one instead of a bare id.
//! Only this module constructs it, so a handler that skips authorization has nothing to hand to
//! storage.
//!
//! - A per-merchant session (JWT or api key) is authorized for its own merchant only. A request
//!   that names any other merchant is refused with `403`.
//! - A service credential (the admin secret, or the `api_key_auth_enabled = false` compat lane)
//!   carries no session and acts on behalf of whichever merchant the request names. This is how
//!   Hyperswitch calls Decision Engine.
//!
//! The merchant is named in the path (read by [`crate::custom_extractors::MerchantFromPath`]) or in
//! the body (read through [`MerchantScoped`] by [`crate::custom_extractors::ScopedJson`], or by
//! handlers that parse the body themselves).

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use super::AuthContext;

/// Path parameters that name the merchant a route acts on. Routing rules are owned by
/// `created_by`, which is the merchant id under another name.
pub const MERCHANT_PATH_PARAMS: &[&str] = &["merchant-id", "merchant_id", "created_by"];

/// A request that names the merchant(s) it acts on.
///
/// Implemented next to each request type that carries a merchant in its body. List *every*
/// merchant the handler reads from the request — a merchant left out here is one a session can
/// reach across to.
pub trait MerchantScoped {
    fn merchant_ids(&self) -> Vec<&str>;
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

/// Resolve the merchant a body-addressed request may act on.
///
/// Every merchant the request names must be the same one. A session must name only its own
/// merchant, and a request that names none acts on the session's. A service credential acts on the
/// merchant the request names.
pub fn authorize(
    session: Option<&AuthContext>,
    request: &impl MerchantScoped,
) -> Result<AuthorizedMerchant, MerchantScopeViolation> {
    let named = request.merchant_ids();
    if let Some(session) = session {
        if named
            .iter()
            .any(|merchant_id| *merchant_id != session.merchant_id)
        {
            return Err(MerchantScopeViolation::ForeignMerchant);
        }
        return Ok(AuthorizedMerchant(session.merchant_id.clone()));
    }
    let Some((first, rest)) = named.split_first() else {
        return Err(MerchantScopeViolation::NoMerchant);
    };
    if rest.iter().any(|merchant_id| merchant_id != first) {
        return Err(MerchantScopeViolation::ConflictingMerchants);
    }
    Ok(AuthorizedMerchant((*first).to_string()))
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
) -> Result<AuthorizedMerchant, MerchantScopeViolation> {
    struct Named<'a>(&'a str);
    impl MerchantScoped for Named<'_> {
        fn merchant_ids(&self) -> Vec<&str> {
            vec![self.0]
        }
    }
    authorize(session, &Named(merchant_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Names(Vec<&'static str>);

    impl MerchantScoped for Names {
        fn merchant_ids(&self) -> Vec<&str> {
            self.0.clone()
        }
    }

    fn authorized(
        session: Option<&str>,
        names: Vec<&'static str>,
    ) -> Result<String, MerchantScopeViolation> {
        let session = session.map(AuthContext::from_api_key);
        authorize(session.as_ref(), &Names(names)).map(|merchant| merchant.as_str().to_string())
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
