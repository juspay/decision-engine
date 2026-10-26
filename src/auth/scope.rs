//! Binding a request to the merchant its session belongs to.
//!
//! A per-merchant session (JWT or api key) may only act on its own merchant. Path-addressed routes
//! are bound in the `enforce_merchant_scope` middleware; body-addressed routes are bound here, on
//! the typed request, so the check reads the same fields — after the same serde renames and
//! aliases — that the handler acts on.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use super::AuthContext;

/// A request that names the merchant(s) it acts on.
///
/// Implemented next to each request type that carries a merchant in its body. List *every*
/// merchant the handler reads from the request — a merchant left out here is one a session can
/// reach across to.
pub trait MerchantScoped {
    fn merchant_ids(&self) -> Vec<&str>;
}

/// A per-merchant session named a merchant other than its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MerchantScopeViolation;

impl MerchantScopeViolation {
    pub const MESSAGE: &'static str = "This session may only act on its own merchant";
}

impl IntoResponse for MerchantScopeViolation {
    fn into_response(self) -> Response {
        crate::error::text_error_response(StatusCode::FORBIDDEN, Self::MESSAGE)
    }
}

/// Refuse a request that names a merchant other than the session's.
///
/// `None` is a service credential (the admin secret, or the `api_key_auth_enabled = false` compat
/// lane): a trusted caller that may act on any merchant.
pub fn ensure_merchant_scope(
    session: Option<&AuthContext>,
    request: &impl MerchantScoped,
) -> Result<(), MerchantScopeViolation> {
    let Some(session) = session else {
        return Ok(());
    };
    if request
        .merchant_ids()
        .into_iter()
        .all(|merchant_id| merchant_id == session.merchant_id)
    {
        Ok(())
    } else {
        Err(MerchantScopeViolation)
    }
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

    #[test]
    fn a_session_may_only_name_its_own_merchant() {
        let session = AuthContext::from_api_key("merchant_a");
        assert_eq!(
            ensure_merchant_scope(Some(&session), &Names(vec!["merchant_a"])),
            Ok(())
        );
        assert_eq!(
            ensure_merchant_scope(Some(&session), &Names(vec!["merchant_b"])),
            Err(MerchantScopeViolation)
        );
    }

    #[test]
    fn every_named_merchant_must_be_the_sessions() {
        let session = AuthContext::from_api_key("merchant_a");
        assert_eq!(
            ensure_merchant_scope(Some(&session), &Names(vec!["merchant_a", "merchant_b"])),
            Err(MerchantScopeViolation)
        );
    }

    #[test]
    fn service_credentials_are_unrestricted() {
        assert_eq!(
            ensure_merchant_scope(None, &Names(vec!["merchant_b"])),
            Ok(())
        );
    }
}
