use ds_identity_bootstrap_ui::ParticipantContextPanel;
use patternfly_yew::prelude::*;
use yew::prelude::*;
use yewdux::prelude::*;

use crate::store::AppState;

fn current_origin() -> Option<String> {
    web_sys::window()?.location().origin().ok()
}

/// This app's own, deliberately 2-step bootstrap flow: create the
/// participant context, then activate it. `ds_identity_bootstrap_ui`'s
/// full `IdentityBootstrapWizard` (create -> activate -> publish DID ->
/// keypair management) is NOT used here -- its `PublishDid` step and the
/// keypair-lifecycle panel it shows on completion both call APIs
/// (`extensions:api:identity-api:did-api` / `...:keypair-api`) that are
/// only registered on EDC's plain IdentityHub/wallet launcher
/// (`identityhub-base-bom`), confirmed absent from the Issuer Service
/// launcher this app talks to (`issuerservice-base-bom` depends on
/// `participant-context-api` but neither of those two) by reading that
/// bom's actual `build.gradle.kts` at the pinned v0.18.0 tag. Calling them
/// against this deployment always 404s, which is exactly what surfaced
/// this (`GET .../dids/state` failing live).
///
/// Activation alone is sufficient on this role: EDC's own
/// `DidDocumentServiceImpl` listens for the `ParticipantContextUpdated`
/// event that activating a participant fires, and auto-publishes its DID
/// as a side effect -- confirmed in source, and confirmed live (the
/// bootstrapped `super-user` context's DID resolves at
/// `https://did.ds-labs.org/super-user/did.json` immediately after
/// activation, with no separate publish call ever made).
///
/// `ParticipantContextPanel` is reused directly for both steps (its own
/// two modes, chosen by whether `participant_context_id` is `None` or
/// `Some`) -- see its doc comment. `ds_identity_bootstrap_ui`'s
/// `DidPublishPanel`/`KeypairLifecyclePanel` and the composed
/// `IdentityBootstrapWizard` stay available in that crate unchanged, for a
/// future consumer targeting the plain IdentityHub/wallet role instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    CreateContext,
    Activate,
    Done,
}

fn step_status(step: Step, current: Step) -> ProgressStepperStepStatus {
    match (step, current) {
        (Step::CreateContext, Step::Activate | Step::Done) => ProgressStepperStepStatus::Success,
        (Step::Activate, Step::Done) => ProgressStepperStepStatus::Success,
        (s, c) if s == c => ProgressStepperStepStatus::Info,
        _ => ProgressStepperStepStatus::Pending,
    }
}

/// Also reachable as the app's own startup redirect target -- see
/// `main.rs`'s `App` component, which routes here itself (rather than
/// rendering `Shell`) when the authority isn't bootstrapped yet.
#[function_component(IdentityBootstrap)]
pub fn identity_bootstrap() -> Html {
    let (app_state, dispatch) = use_store::<AppState>();
    let participant_id = use_state(|| app_state.selected_participant_context.clone());
    let step = use_state(|| match &*participant_id {
        Some(_) => Step::Activate,
        None => Step::CreateContext,
    });

    let Some(endpoint) = current_origin() else {
        return html! {
            <PageSection>
                <Content>
                    <p>{ "Could not determine the page origin." }</p>
                </Content>
            </PageSection>
        };
    };

    let on_created = {
        let step = step.clone();
        let participant_id = participant_id.clone();
        Callback::from(move |created_id: String| {
            participant_id.set(Some(created_id));
            step.set(Step::Activate);
        })
    };

    // Once activation succeeds this becomes the app-wide selected
    // participant context, same as every other route view already reads
    // from `AppState` -- so leaving this view (nav, or the startup
    // redirect unwinding) lands on a normally-populated Dashboard/Holders/
    // etc rather than an empty-state prompting the user to pick one again.
    let on_activated = {
        let step = step.clone();
        let participant_id = participant_id.clone();
        let dispatch = dispatch.clone();
        Callback::from(move |()| {
            step.set(Step::Done);
            if let Some(id) = &*participant_id {
                dispatch.reduce_mut(|state| state.selected_participant_context = Some(id.clone()));
            }
        })
    };

    let stepper = html! {
        <ProgressStepper>
            <ProgressStepperStep
                status={step_status(Step::CreateContext, *step)}
                is_current={*step == Step::CreateContext}
                description="Establish the identity and its first signing key"
            >
                { "Create context" }
            </ProgressStepperStep>
            <ProgressStepperStep
                status={step_status(Step::Activate, *step)}
                is_current={*step == Step::Activate}
                description="Turns the identity on and publishes its DID"
            >
                { "Activate" }
            </ProgressStepperStep>
        </ProgressStepper>
    };

    let body = match *step {
        Step::CreateContext => html! {
            <ParticipantContextPanel
                endpoint={endpoint.clone()}
                participant_context_id={None::<String>}
                on_created={on_created}
            />
        },
        Step::Activate => {
            let Some(id) = (*participant_id).clone() else {
                return html!(
                    <Alert r#type={AlertType::Danger} title="No participant context to activate" inline=true />
                );
            };
            html! {
                <ParticipantContextPanel
                    endpoint={endpoint.clone()}
                    participant_context_id={Some(id)}
                    on_activated={on_activated}
                />
            }
        }
        Step::Done => {
            let id = (*participant_id).clone().unwrap_or_default();
            html! {
                <Alert r#type={AlertType::Success} title="Identity bootstrapped" inline=true>
                    <p>{ format!("\"{id}\" is active and its DID is published.") }</p>
                </Alert>
            }
        }
    };

    html! {
        <PageSection>
            <Content>
                <Title level={Level::H1} size={Size::XXLarge}>{ "Identity Bootstrap" }</Title>
                <p>{ "Create the authority's participant context and activate it -- activation publishes its DID automatically." }</p>
            </Content>
            { stepper }
            <br />
            { body }
        </PageSection>
    }
}
