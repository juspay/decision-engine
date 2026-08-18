#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthKind {
    Jwt,
    ApiKey,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthContext {
    pub merchant_id: String,
    pub auth_kind: AuthKind,
    pub user_id: Option<String>,
    pub email: Option<String>,
    pub role: Option<String>,
    /// What this session may do. `None` means unrestricted — a service credential, which has no
    /// dashboard role to limit. A session carries the resolved list, never `None`.
    ///
    /// Checked in the authenticate middleware, so every route is covered at once rather than
    /// depending on each handler to remember. Handlers may consult it for finer decisions.
    pub permissions: Option<Vec<super::Permission>>,
}

impl AuthContext {
    pub fn from_jwt(claims: &super::JwtClaims, require_explicit_permissions: bool) -> Self {
        Self {
            merchant_id: claims.merchant_id.clone(),
            auth_kind: AuthKind::Jwt,
            user_id: Some(claims.user_id.clone()),
            email: Some(claims.email.clone()),
            role: Some(claims.role.clone()),
            // Resolved by the claims themselves rather than re-derived here, so the middleware
            // enforces exactly what `/auth/me` reports.
            permissions: Some(claims.permissions(require_explicit_permissions)),
        }
    }

    /// An API key is a service credential with no person behind it, so there is no dashboard role
    /// to limit. Keys are scoped by merchant, and revoked rather than downgraded.
    pub fn from_api_key(merchant_id: impl Into<String>) -> Self {
        Self {
            merchant_id: merchant_id.into(),
            auth_kind: AuthKind::ApiKey,
            user_id: None,
            email: None,
            role: None,
            permissions: None,
        }
    }

    /// Whether this session holds `permission`. An unrestricted session holds everything.
    pub fn allows(&self, permission: &super::Permission) -> bool {
        match &self.permissions {
            Some(held) => held.contains(permission),
            None => true,
        }
    }
}
