use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Authentication required: missing or invalid token")]
    Unauthenticated,

    #[error("Forbidden: insufficient permissions")]
    Forbidden,

    #[error("Invalid token: {0}")]
    InvalidToken(String),

    #[error("Token validation failed: {0}")]
    ValidationFailed(String),

    #[error("JWKS fetch failed: {0}")]
    JwksFetchFailed(String),

    #[error("Issuer mismatch: expected {expected}, got {actual}")]
    IssuerMismatch { expected: String, actual: String },

    #[error("Audience mismatch: expected {expected:?}, got {actual:?}")]
    AudienceMismatch {
        expected: Vec<String>,
        actual: Vec<String>,
    },

    #[error("Token expired")]
    TokenExpired,

    #[error("Internal error: {0}")]
    Internal(String),
}
