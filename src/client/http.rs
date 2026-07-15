use wreq::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, CONTENT_TYPE, ORIGIN as ORIGIN_HDR, REFERER as REFERER_HDR};

use crate::errors::{MspError, Result};
use serde_json::Value;

pub const ORIGIN:  &str = "https://moviestarplanet2.com";
pub const REFERER: &str = "https://moviestarplanet2.com/";

const APPLICATION_JSON: &str = "application/json";
const FORM_URLENCODED:  &str = "application/x-www-form-urlencoded";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentType {
    Form,
    Json,
}

impl ContentType {
    #[inline]
    const fn content_type(self) -> &'static str {
        match self {
            ContentType::Form => FORM_URLENCODED,
            ContentType::Json => APPLICATION_JSON,
        }
    }
}


#[must_use]
pub fn build_headers(kind: ContentType, bearer: Option<&str>) -> HeaderMap {
    let mut h = HeaderMap::with_capacity(5);

    h.insert(CONTENT_TYPE, HeaderValue::from_static(kind.content_type()));
    h.insert(ACCEPT, HeaderValue::from_static(APPLICATION_JSON));
    h.insert(REFERER_HDR, HeaderValue::from_static(REFERER));

    if kind == ContentType::Form {
        h.insert(ORIGIN_HDR, HeaderValue::from_static(ORIGIN));
    }

    if let Some(token) = bearer {
        match HeaderValue::from_str(token) {
            Ok(mut value) => {
                value.set_sensitive(true);
                h.insert(AUTHORIZATION, value);
            }
            Err(e) => {
                tracing::warn!(
                    "Skipping Authorization header — token is not a valid header value: {e}"
                );
            }
        }
    }

    h
}


#[inline]
pub fn ensure_no_error(value: Value) -> Result<Value> {
    let Some(err) = value.get("error") else {
        return Ok(value);
    };

    let status = value
        .get("statusCode")
        .or_else(|| value.get("status"))
        .and_then(Value::as_u64)
        .map(|s| s as u16)
        .unwrap_or(400);

    let error = err.as_str().map(str::to_owned).unwrap_or_else(|| err.to_string());

    let body = match value.get("error_description").and_then(Value::as_str) {
        Some(desc) => format!("{error}: {desc}"),
        None => error,
    };

    Err(MspError::Api { status, body })
}


#[inline]
pub fn required_str<'v>(value: &'v Value, field: &str) -> Result<&'v str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| missing_field(field))
}


#[inline]
pub fn required_i64(value: &Value, field: &str) -> Result<i64> {
    value
        .get(field)
        .and_then(Value::as_i64)
        .ok_or_else(|| missing_field(field))
}


#[inline]
pub fn required_bool(value: &Value, field: &str) -> Result<bool> {
    value
        .get(field)
        .and_then(Value::as_bool)
        .ok_or_else(|| missing_field(field))
}


#[inline]
#[must_use]
pub fn optional_str<'v>(value: &'v Value, field: &str) -> Option<&'v str> {
    value.get(field).and_then(Value::as_str)
}

#[inline]
fn missing_field(field: &str) -> MspError {
    MspError::Api {
        status: 422,
        body: format!("Missing or invalid field '{field}' in response structure"),
    }
}