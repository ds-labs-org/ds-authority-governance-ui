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

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Config {
    pub identity_api_path: String,
    pub issuer_admin_api_path: String,
    /// Same-origin path the gateway forwards to the trusted issuers
    /// registry. Optional, defaults to `/api/registry`.
    #[serde(default = "default_registry_api_path")]
    pub registry_api_path: String,
    /// The hub's participant context id (sent base64-encoded in every
    /// `v1alpha` path). Optional, defaults to `issuer`.
    #[serde(default = "default_participant_context_id")]
    pub participant_context_id: String,
    #[serde(default)]
    pub bearer_token: Option<String>,
}

pub const DEFAULT_REGISTRY_API_PATH: &str = "/api/registry";
pub const DEFAULT_PARTICIPANT_CONTEXT_ID: &str = "issuer";

fn default_registry_api_path() -> String {
    DEFAULT_REGISTRY_API_PATH.to_string()
}

fn default_participant_context_id() -> String {
    DEFAULT_PARTICIPANT_CONTEXT_ID.to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            identity_api_path: "/api/identity".to_string(),
            issuer_admin_api_path: "/api/issuer".to_string(),
            registry_api_path: default_registry_api_path(),
            participant_context_id: default_participant_context_id(),
            bearer_token: None,
        }
    }
}

impl Config {
    /// Adds the `Authorization: Bearer <token>` header to `builder` when
    /// `bearer_token` is set, leaving `builder` untouched otherwise. Every
    /// API call this app builds should route through this so a configured
    /// bearer token is applied consistently, on top of (not instead of) the
    /// browser's own same-origin session cookie.
    /// `src/api/client.rs` routes every request through here.
    pub fn authorize(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.bearer_token {
            Some(token) => builder.bearer_auth(token),
            None => builder,
        }
    }
}

/// Fetches and parses `configuration.json` from `origin` (the page's own
/// origin, from `web_sys::window().location().origin()`).
pub async fn fetch_config(origin: &str) -> Result<Config, String> {
    let response = reqwest::get(format!("{origin}/configuration.json"))
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

    #[test]
    fn registry_path_and_participant_context_default_when_absent() {
        let config: Config = serde_json::from_str(
            r#"{"identity_api_path": "/api/identity", "issuer_admin_api_path": "/api/issuer"}"#,
        )
        .unwrap();

        assert_eq!(config.registry_api_path, "/api/registry");
        assert_eq!(config.participant_context_id, "issuer");
        assert_eq!(config, Config::default());
    }

    #[test]
    fn registry_path_and_participant_context_are_read_when_present() {
        let config: Config = serde_json::from_str(
            r#"{"identity_api_path": "/i", "issuer_admin_api_path": "/a", "registry_api_path": "/r", "participant_context_id": "pc"}"#,
        )
        .unwrap();

        assert_eq!(config.registry_api_path, "/r");
        assert_eq!(config.participant_context_id, "pc");
    }

    #[test]
    fn shipped_configuration_json_parses_with_contract_values() {
        let config: Config = serde_json::from_str(include_str!("../configuration.json")).unwrap();

        assert_eq!(config, Config::default());
    }
}
