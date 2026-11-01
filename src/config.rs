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
//! This app carries no credentials. It is served behind an OIDC gate
//! (apisix + Zitadel) that sets a same-origin session cookie, which the
//! browser sends on every API call, and the gateway adds the APIs' own
//! admin credentials. `configuration.json` is a static file any visitor
//! can read, so a `bearer_token` in it would be public: a file that sets
//! one is refused (2026-10-16 audit, F6). An explicit `null`, as older
//! files carry, is accepted.

use serde::Deserialize;
use serde::de::IgnoredAny;

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(try_from = "RawConfig")]
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
}

/// `configuration.json` as written, before [`Config`] refuses a token in it.
#[derive(Deserialize)]
struct RawConfig {
    identity_api_path: String,
    issuer_admin_api_path: String,
    #[serde(default = "default_registry_api_path")]
    registry_api_path: String,
    #[serde(default = "default_participant_context_id")]
    participant_context_id: String,
    #[serde(default)]
    bearer_token: Option<IgnoredAny>,
}

impl TryFrom<RawConfig> for Config {
    type Error = &'static str;

    fn try_from(raw: RawConfig) -> Result<Self, Self::Error> {
        if raw.bearer_token.is_some() {
            return Err(
                "configuration.json must not set bearer_token: every visitor can read \
                 this file, so remove the token and rotate it; the gateway adds the APIs' \
                 credentials",
            );
        }
        Ok(Self {
            identity_api_path: raw.identity_api_path,
            issuer_admin_api_path: raw.issuer_admin_api_path,
            registry_api_path: raw.registry_api_path,
            participant_context_id: raw.participant_context_id,
        })
    }
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
        }
    }
}

/// Fetches and parses `configuration.json` from `origin` (the page's own
/// origin, from `web_sys::window().location().origin()`). The error names
/// what is wrong with the file, a `bearer_token` included.
pub async fn fetch_config(origin: &str) -> Result<Config, String> {
    let response = reqwest::get(format!("{origin}/configuration.json"))
        .await
        .map_err(|error| error.to_string())?;
    let text = response.text().await.map_err(|error| error.to_string())?;
    serde_json::from_str(&text).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_api_paths_are_read() {
        let config: Config = serde_json::from_str(
            r#"{"identity_api_path": "/api/identity", "issuer_admin_api_path": "/api/issuer"}"#,
        )
        .unwrap();

        assert_eq!(config.identity_api_path, "/api/identity");
        assert_eq!(config.issuer_admin_api_path, "/api/issuer");
    }

    /// `configuration.json` is a static file any visitor can read, so a
    /// token in it is public (2026-10-16 audit, F6). The gateway adds the
    /// admin credentials: a `bearer_token` here is refused, never sent.
    #[test]
    fn a_bearer_token_in_the_configuration_is_refused() {
        let error = serde_json::from_str::<Config>(
            r#"{"identity_api_path": "/api/identity", "issuer_admin_api_path": "/api/issuer", "bearer_token": "secret"}"#,
        )
        .unwrap_err();
        assert!(error.to_string().contains("bearer_token"), "{error}");

        let config: Config = serde_json::from_str(
            r#"{"identity_api_path": "/api/identity", "issuer_admin_api_path": "/api/issuer", "bearer_token": null}"#,
        )
        .expect("an explicit null, as older configuration.json files carry, is accepted");
        assert_eq!(config, Config::default());
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
