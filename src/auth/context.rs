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
}

impl AuthContext {
    pub fn from_jwt(claims: &super::JwtClaims) -> Self {
        Self {
            merchant_id: claims.merchant_id.clone(),
            auth_kind: AuthKind::Jwt,
            user_id: Some(claims.user_id.clone()),
            email: Some(claims.email.clone()),
            role: Some(claims.role.clone()),
        }
    }

    pub fn from_api_key(merchant_id: impl Into<String>) -> Self {
        Self {
            merchant_id: merchant_id.into(),
            auth_kind: AuthKind::ApiKey,
            user_id: None,
            email: None,
            role: None,
        }
    }
}
