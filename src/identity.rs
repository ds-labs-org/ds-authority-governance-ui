//! The signed-in Zitadel user, fetched via apisix's `/api/userinfo` route
//! (`roles/edc_issuer`'s `apisix-routes.yaml.j2`) -- gated by the same
//! `openid-connect` plugin as `/api/identity`/`/api/issuer`, so an
//! unauthenticated request never reaches its handler at all: apisix
//! 302s to Zitadel first, and only injects the `X-Userinfo` header (which
//! that route echoes back as JSON) once a session is established.
//!
//! A plain `fetch` that hits that redirect chain follows it transparently
//! and returns Zitadel's login *page* (HTML, status 200) -- so "the
//! request succeeded" is NOT the right signal for "the user is
//! authenticated". The right signal is "the response actually parsed as
//! `UserInfo` JSON", checked here by attempting exactly that.

use serde::Deserialize;

/// Standard OIDC userinfo claims Zitadel returns. Only `sub` is
/// guaranteed; the rest depend on the requested scope (`openid profile
/// email`, per apisix-routes.yaml.j2) and are optional here rather than
/// assumed present.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct UserInfo {
    pub sub: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub preferred_username: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
}

/// Fetches `/api/userinfo`, resolved against the document's base URI --
/// same reasoning as `config::fetch_config` (a bare relative string
/// fails at reqwest's own URL-parsing layer before ever reaching the
/// browser). Returns `Err` for both a transport failure and "the
/// response wasn't actually UserInfo JSON" -- callers can't tell those
/// apart from this alone, which is fine: either way the caller's next
/// step is the same, force_login_redirect below.
pub async fn fetch_userinfo() -> Result<UserInfo, String> {
    let base = crate::config::document_base_uri()
        .ok_or_else(|| "could not determine the document's base URI".to_string())?;

    let response = reqwest::get(format!("{base}api/userinfo"))
        .await
        .map_err(|error| error.to_string())?;

    response
        .json::<UserInfo>()
        .await
        .map_err(|error| error.to_string())
}

/// Forces a REAL top-level page navigation to `/api/userinfo` (not
/// another `fetch`) so the browser actually shows Zitadel's login form --
/// a `fetch` redirect is followed silently, with nothing rendered.
/// Passes the current base URI as `return_to`: once authenticated, the
/// route's own serverless-pre-function redirects back here (a plain
/// redirect at that point, session already established, not another
/// auth round-trip) instead of stranding the user on the bare JSON
/// response.
///
/// Known limitation: `return_to` is the app's base URI (e.g.
/// `https://issuer-admin.ds-labs.org/ux/`), not the current hash route --
/// the URL fragment never reaches the server, so a deep link
/// (`/ux/#/holders`) is not restored after a fresh login; the user lands
/// on the Dashboard and re-navigates. Acceptable for now; revisit if it
/// proves annoying in practice.
pub fn force_login_redirect() {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(base) = crate::config::document_base_uri() else {
        return;
    };
    let target = format!(
        "{base}api/userinfo?return_to={}",
        urlencoding_encode(&base)
    );
    let _ = window.location().set_href(&target);
}

/// Minimal percent-encoding for a URL query-parameter value -- avoids
/// pulling in a whole crate (`urlencoding`/`percent-encoding`) for one
/// call site. `base` is always a `document.baseURI` value (scheme +
/// host + path, no query/fragment of its own), so this only ever needs
/// to escape the characters that would otherwise break out of the
/// `return_to=` query value: `:`, `/`, and nothing else appears in a
/// base URI.
fn urlencoding_encode(value: &str) -> String {
    value.replace(':', "%3A").replace('/', "%2F")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn userinfo_deserializes_with_only_sub_present() {
        let info: UserInfo = serde_json::from_str(r#"{"sub": "abc123"}"#).unwrap();
        assert_eq!(info.sub, "abc123");
        assert_eq!(info.name, None);
    }

    #[test]
    fn userinfo_deserializes_full_profile() {
        let info: UserInfo = serde_json::from_str(
            r#"{"sub": "abc123", "name": "Nico", "preferred_username": "nico", "email": "nico@ds-labs.org"}"#,
        )
        .unwrap();
        assert_eq!(info.name.as_deref(), Some("Nico"));
        assert_eq!(info.email.as_deref(), Some("nico@ds-labs.org"));
    }

    #[test]
    fn base_uri_is_percent_encoded_for_the_query_value() {
        assert_eq!(
            urlencoding_encode("https://issuer-admin.ds-labs.org/ux/"),
            "https%3A%2F%2Fissuer-admin.ds-labs.org%2Fux%2F"
        );
    }
}
