use patternfly_yew::prelude::PageSection;
use yew::prelude::*;

// TODO: uncomment once ds-identity-bootstrap-ui ships its components
// use ds_identity_bootstrap_ui::ParticipantContextPanel;

/// Placeholder for the `/identity-bootstrap` route.
///
/// This will eventually mount `ds_identity_bootstrap_ui::ParticipantContextPanel`.
/// That crate's `lib.rs` currently only declares stub `mod` items
/// (`participant_context_panel`, `did_publish_panel`,
/// `keypair_lifecycle_panel`) with no component files behind them yet, so
/// depending on it as a *value* would not compile. The `git` dependency
/// stays in `Cargo.toml` (do not remove it) and this route/nav entry is
/// wired up now so no further routing work is needed once that crate ships
/// real components - just uncomment the import and usage below.
#[function_component(IdentityBootstrap)]
pub fn identity_bootstrap() -> Html {
    html! {
        <PageSection>
            // TODO: uncomment once ds-identity-bootstrap-ui ships its components
            // <ParticipantContextPanel />
            <p>{ "TODO" }</p>
        </PageSection>
    }
}
