use ds_identity_bootstrap_ui::IdentityBootstrapWizard;
use patternfly_yew::prelude::{Content, Level, PageSection, Size, Title};
use yew::prelude::*;
use yewdux::prelude::*;

use crate::store::AppState;

fn current_origin() -> Option<String> {
    web_sys::window()?.location().origin().ok()
}

/// The `/identity-bootstrap` route: mounts `ds_identity_bootstrap_ui`'s
/// real `IdentityBootstrapWizard`, now that crate ships actual components
/// (`ds-identity-bootstrap-ui`'s `feat/wizard-components` branch) instead
/// of the placeholder `ParticipantContextPanel`-only stub this view used to
/// mention.
///
/// Also reachable as the app's own startup redirect target -- see
/// `main.rs`'s `App` component, which routes here itself (rather than
/// rendering `Shell`) when no DID is published yet.
#[function_component(IdentityBootstrap)]
pub fn identity_bootstrap() -> Html {
    let (app_state, dispatch) = use_store::<AppState>();
    let participant_context_id = app_state.selected_participant_context.clone();
    let bearer_token = app_state
        .config
        .as_ref()
        .and_then(|config| config.bearer_token.clone());

    let Some(endpoint) = current_origin() else {
        return html! {
            <PageSection>
                <Content>
                    <p>{ "Could not determine the page origin." }</p>
                </Content>
            </PageSection>
        };
    };

    // Once the wizard reaches "Done" this becomes the app-wide selected
    // participant context, same as every other route view already reads
    // from `AppState` -- so leaving the wizard (nav, or the startup
    // redirect unwinding) lands on a normally-populated Dashboard/Holders/
    // etc rather than an empty-state prompting the user to pick one again.
    let on_bootstrapped = {
        let dispatch = dispatch.clone();
        Callback::from(move |id: String| {
            dispatch.reduce_mut(|state| state.selected_participant_context = Some(id));
        })
    };

    html! {
        <PageSection>
            <Content>
                <Title level={Level::H1} size={Size::XXLarge}>{ "Identity Bootstrap" }</Title>
                <p>{ "Create the participant context, activate it, and publish its DID -- the same three steps whether this is an authority or a plain connector." }</p>
            </Content>
            <IdentityBootstrapWizard
                {endpoint}
                {bearer_token}
                {participant_context_id}
                {on_bootstrapped}
            />
        </PageSection>
    }
}
