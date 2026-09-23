//! The app's route table. Rendered via `yew_router::HashRouter` -
//! hash-based routing only (URLs like `/#/holders`), never history/browser
//! mode, so the built bundle can be dropped on any static host/CDN with no
//! server-side fallback-rule configuration.

use yew_router::Routable;

#[derive(Clone, Debug, PartialEq, Routable)]
pub enum Route {
    #[at("/")]
    Dashboard,
    #[at("/credential-definitions")]
    CredentialDefinitions,
    #[at("/holders")]
    Holders,
    #[at("/credentials")]
    Credentials,
    #[at("/identity-bootstrap")]
    IdentityBootstrap,
}
