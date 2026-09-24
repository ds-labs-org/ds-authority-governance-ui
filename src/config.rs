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
//! There is deliberately no client-side API key/bearer-token field here
//! anymore. EDC's identity-api/issuer-admin-api require their own
//! `x-api-key` header underneath apisix's Zitadel gate (see
//! `edc-identity-hub-client`'s `IdentityHubClient`/`IssuerAdminApiClient`,
//! both constructed with `None` throughout this app) -- that key is the
//! bootstrapped EDC "super-user"'s, with admin scope over every
//! participant, credential-definition and DID. `configuration.json` is
//! served by this app's own unauthenticated static hosting (no OIDC gate
//! applies to it -- see roles/edc_issuer's apisix static `/ux/` location in
//! dslabs-infra), so shipping that key here would leak it to anyone who
//! loads the page, logged in or not. apisix now injects it server-side
//! (`proxy-rewrite`, dslabs-infra's `apisix-routes.yaml.j2`) on top of its
//! existing Zitadel gate, after authenticating the human -- the browser
//! never needs to see or send it.

use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct Config {
    pub identity_api_path: String,
    pub issuer_admin_api_path: String,
}

/// The document's resolved, absolute base URI (`document.baseURI`) --
/// honors the `<base href>` tag Trunk sets from `public_url` (e.g.
/// `https://issuer-admin.ds-labs.org/ux/`), already fully resolved by the
/// browser (unlike `<base href>`'s own attribute value, which could be a
/// bare relative path).
pub(crate) fn document_base_uri() -> Option<String> {
    web_sys::window()?.document()?.base_uri().ok().flatten()
}

/// The page's origin (scheme + host + port, e.g.
/// `https://issuer-admin.ds-labs.org` -- no trailing slash, no path).
///
/// NOT the same thing as `document_base_uri()` above, and the two are NOT
/// interchangeable: `document_base_uri()` includes this app's own `/ux/`
/// path prefix (correct for resolving a file co-located with the bundle,
/// like `configuration.json`), while apisix routes such as
/// `/api/identity`, `/api/issuer`, and `/api/userinfo` are origin-rooted
/// paths that live OUTSIDE `/ux/` entirely -- prefixing them with the
/// base URI produces a wrong, nonexistent `/ux/api/...` URL. Every
/// origin-rooted fetch/navigation in this app must build its URL from
/// `document_origin()`, not `document_base_uri()`.
pub(crate) fn document_origin() -> Option<String> {
    web_sys::window()?.location().origin().ok()
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
    fn config_deserializes_the_two_api_paths() {
        let config: Config = serde_json::from_str(
            r#"{"identity_api_path": "/api/identity", "issuer_admin_api_path": "/api/issuer"}"#,
        )
        .unwrap();

        assert_eq!(config.identity_api_path, "/api/identity");
        assert_eq!(config.issuer_admin_api_path, "/api/issuer");
    }
}
