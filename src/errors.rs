use thiserror::Error;

pub type Result<T, E = MspError> = std::result::Result<T, E>;

#[derive(Debug, Error)]
pub enum MspError {
    #[error("Authentication failed: {0}")]
    Authentication(String),

    #[error("No active session – please authenticate first")]
    NoSession,

    #[error("Network transfer error: {0}")]
    Network(#[from] wreq::Error),

    #[error("JSON processing error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("JWT parsing error: {0}")]
    Jwt(String),

    #[error("MSP API protocol error (HTTP {status}): {body}")]
    Api { status: u16, body: String },

    #[error("Base64 decoding failed: {0}")]
    Base64(#[from] base64::DecodeError),

    #[error("URL serialization failed: {0}")]
    UrlEncoded(#[from] serde_urlencoded::ser::Error),

    #[error("Invalid proxy configuration: {0}")]
    InvalidProxy(String),
}