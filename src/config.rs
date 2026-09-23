//! Runtime configuration for this app, fetched from `configuration.json` at
//! the app's own origin at startup - same pattern as
//! `ds-catalog-browser-ui`'s `src/configuration.rs`.
//!
//! `identity_api_path` and `issuer_admin_api_path` are **same-origin,
//! relative paths** (e.g. `/api/identity`), never full URLs with a host: a
//! reverse proxy in front of this app (apisix, in this project's
//! deployment) is expected to forward each path to the real identity-api /
//! issuer-admin-api, so the browser never has to make a cross-origin
//! request.
//!
//! `bearer_token` is optional. This app is served behind an OIDC gate
//! (apisix + Zitadel) that sets a same-origin session cookie, which the
//! browser sends automatically on every API call - that's the common case
//! and needs nothing from this struct. `bearer_token`, when present, is an
//! *additional* `Authorization: Bearer <token>` header for scripted/service
//! access; it never replaces the cookie.

use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct Config {
    pub identity_api_path: String,
    pub issuer_admin_api_path: String,
    #[serde(default)]
    pub bearer_token: Option<String>,
}

impl Config {
    /// Adds the `Authorization: Bearer <token>` header to `builder` when
    /// `bearer_token` is set, leaving `builder` untouched otherwise. Every
    /// API call this app builds should route through this so a configured
    /// bearer token is applied consistently, on top of (not instead of) the
    /// browser's own same-origin session cookie.
    ///
    /// Not called anywhere yet - the shell doesn't make any API calls of its
    /// own. Kept here (rather than dead-code-warned away) for the route
    /// views that will build the actual identity-api / issuer-admin-api
    /// clients.
    #[allow(dead_code)]
    pub fn authorize(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.bearer_token {
            Some(token) => builder.bearer_auth(token),
            None => builder,
        }
    }
}

/// The document's resolved, absolute base URI (`document.baseURI`) --
/// honors the `<base href>` tag Trunk sets from `public_url` (e.g.
/// `https://issuer-admin.ds-labs.org/ux/`), already fully resolved by the
/// browser (unlike `<base href>`'s own attribute value, which could be a
/// bare relative path).
fn document_base_uri() -> Option<String> {
    web_sys::window()?.document()?.base_uri().ok().flatten()
}

/// Fetches and parses `configuration.json`, resolved against the
/// document's own base URI -- NOT an origin-absolute
/// `{origin}/configuration.json` path, which lands at the origin's root
/// regardless of what prefix this app is actually served under (this app
/// is deployed under `/ux/`, roles/edc_issuer's apisix static-serving
/// location).
///
/// Also NOT a bare relative `reqwest::get("configuration.json")` --
/// tried that first, and it fails with a reqwest "builder error" before
/// any network call: reqwest builds its own `url::Url` via `IntoUrl`,
/// which requires an absolute URL (`Url::parse` errors on a schemeless,
/// hostless string with `RelativeUrlWithoutBase`) -- it does NOT hand the
/// raw string to the browser's `fetch()` for the browser to resolve
/// against `<base href>` itself. The resolution has to happen on our side
/// first, via `document_base_uri()` above.
pub async fn fetch_config() -> Result<Config, String> {
    let base = document_base_uri()
        .ok_or_else(|| "could not determine the document's base URI".to_string())?;
    let response = reqwest::get(format!("{base}configuration.json"))
        .await
        .map_err(|error| error.to_string())?;

    response
        .json::<Config>()
        .await
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bearer_token_defaults_to_none_when_absent() {
        let config: Config = serde_json::from_str(
            r#"{"identity_api_path": "/api/identity", "issuer_admin_api_path": "/api/issuer"}"#,
        )
        .unwrap();

        assert_eq!(config.identity_api_path, "/api/identity");
        assert_eq!(config.issuer_admin_api_path, "/api/issuer");
        assert_eq!(config.bearer_token, None);
    }

    #[test]
    fn bearer_token_is_read_when_present() {
        let config: Config = serde_json::from_str(
            r#"{"identity_api_path": "/api/identity", "issuer_admin_api_path": "/api/issuer", "bearer_token": "secret"}"#,
        )
        .unwrap();

        assert_eq!(config.bearer_token, Some("secret".to_string()));
    }
}
