//! Thin HTTP execution of an [`ApiRequest`]. All decisions (URLs, bodies,
//! error messages) live in the pure, host-tested modules; this only sends
//! bytes through reqwest, with [`Config::authorize`] applied to every call.

use serde::de::DeserializeOwned;

use super::error::ApiError;
use super::requests::{ApiRequest, Method};
use crate::config::Config;

/// Sends `request` and returns `(status, body)` without judging the status.
pub async fn send_raw(config: &Config, request: &ApiRequest) -> Result<(u16, String), ApiError> {
    let method = match request.method {
        Method::Get => reqwest::Method::GET,
        Method::Post => reqwest::Method::POST,
        Method::Put => reqwest::Method::PUT,
        Method::Patch => reqwest::Method::PATCH,
        Method::Delete => reqwest::Method::DELETE,
    };
    let url = format!("{}{}", origin(), request.url);
    let mut builder = reqwest::Client::new()
        .request(method, url)
        .header("Accept", "application/json");
    if let Some(body) = &request.body {
        builder = builder.json(body);
    }
    let response = config
        .authorize(builder)
        .send()
        .await
        .map_err(|e| ApiError::Network(e.to_string()))?;
    let status = response.status().as_u16();
    let text = response
        .text()
        .await
        .map_err(|e| ApiError::Network(e.to_string()))?;
    Ok((status, text))
}

/// Sends `request`, failing on any non-2xx status.
pub async fn send(config: &Config, request: &ApiRequest) -> Result<String, ApiError> {
    let (status, body) = send_raw(config, request).await?;
    if (200..300).contains(&status) {
        Ok(body)
    } else {
        Err(ApiError::from_response(status, &body))
    }
}

/// Sends `request` and parses a 2xx JSON body as `T`.
pub async fn send_json<T: DeserializeOwned>(
    config: &Config,
    request: &ApiRequest,
) -> Result<T, ApiError> {
    let body = send(config, request).await?;
    serde_json::from_str(&body).map_err(|e| ApiError::Decode(e.to_string()))
}

/// Sends `request`, ignoring any 2xx body.
pub async fn send_empty(config: &Config, request: &ApiRequest) -> Result<(), ApiError> {
    send(config, request).await.map(drop)
}

#[cfg(target_arch = "wasm32")]
fn origin() -> String {
    web_sys::window()
        .and_then(|w| w.location().origin().ok())
        .unwrap_or_default()
}

/// reqwest needs absolute URLs; off the browser there is no page origin.
#[cfg(not(target_arch = "wasm32"))]
fn origin() -> String {
    "http://localhost".to_string()
}
