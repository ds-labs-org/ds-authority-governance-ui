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
///
/// Uses `document_origin()`, NOT `document_base_uri()`: `/api/userinfo`
/// is an origin-rooted apisix route, outside this app's own `/ux/` path
/// prefix -- prefixing it with the base URI produces a nonexistent
/// `/ux/api/userinfo` (404), confirmed live before this fix.
pub async fn fetch_userinfo() -> Result<UserInfo, String> {
    let origin = crate::config::document_origin()
        .ok_or_else(|| "could not determine the page origin".to_string())?;

    let response = reqwest::get(format!("{origin}/api/userinfo"))
        .await
        .map_err(|error| error.to_string())?;

    response
        .json::<UserInfo>()
        .await
        .map_err(|error| error.to_string())
}

/// Forces a REAL top-level page navigation to `/api/login` (not another
/// `fetch`) so the browser actually shows Zitadel's login form -- a
/// `fetch` redirect is followed silently, with nothing rendered.
///
/// Deliberately a dedicated route, not `/api/userinfo` with a
/// `?return_to=<url>` query param (tried first): that collided with
/// openid-connect's OWN internal "remember the originally-requested URI,
/// redirect back to it after login" mechanism -- confirmed live, the
/// post-login redirect landed on a mangled, 404ing target rather than
/// actually preserving the query string. `/api/login` carries no query
/// param at all, so openid-connect's own redirect-back always lands on
/// the same bare `/api/login` it started from; that route's own
/// serverless-pre-function then does one plain, hardcoded,
/// unconditional redirect back into the app (`roles/edc_issuer`'s
/// `apisix-routes.yaml.j2`) -- no query-string round-trip involved
/// anywhere in the flow.
///
/// Known limitation: always lands on `/ux/` (the Dashboard), not
/// wherever the user actually was -- the hardcoded redirect target has
/// no way to know the current hash route (which never reaches the
/// server anyway). Acceptable for now; revisit if it proves annoying in
/// practice.
pub fn force_login_redirect() {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(origin) = crate::config::document_origin() else {
        return;
    };
    let _ = window.location().set_href(&format!("{origin}/api/login"));
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
}
