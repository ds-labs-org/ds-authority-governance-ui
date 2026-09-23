//! `/holders` route: deciding which DIDs this authority trusts to request
//! credentials.
//!
//! ## What "approval workflow" means here
//!
//! EDC's own `Holder` domain model (issuer-admin-api, v0.18.0) has **no**
//! pending/approval/lifecycle field at all - it is a plain trust record
//! (`holderId`, `did`, `holderName`, `anonymous`, a free-form `properties`
//! map, `lastModifiedAt`). There is no server-side concept of an incoming
//! registration request waiting on a decision.
//!
//! So this view can only make the "Approved" side real: the **Approved**
//! tab is a working list/create/delete UI backed by the real
//! `POST|GET|DELETE .../holders` endpoints - creating a holder here *is*
//! "approving" it, in the only sense EDC supports (it starts existing as a
//! trusted record; there is nothing to accept or reject afterwards).
//!
//! The **Pending Review** tab is a clearly-labelled placeholder. It shows
//! no data, real or fake, because there is currently no server-side source
//! of "incoming holder requests" to point it at. Wiring it up for real
//! needs a product decision (e.g. an app-level `status` convention inside
//! `Holder.properties`, or a separate request-intake mechanism) - see the
//! session report for this track.
use edc_identity_hub_client::models::{Holder, HolderDto, QuerySpec};
use edc_identity_hub_client::{IdentityHubClientError, IdentityHubClientVersion, IssuerAdminApiClient};
use patternfly_yew::prelude::*;
use yew::platform::spawn_local;
use yew::prelude::*;
use yewdux::prelude::*;

use crate::store::AppState;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HoldersTab {
    Approved,
    Pending,
}

/// Placeholder for the outcome of a data-fetching effect.
#[derive(Clone, PartialEq)]
enum LoadState<T> {
    Loading,
    Loaded(T),
    Error(String),
}

fn browser_origin() -> Option<String> {
    web_sys::window()?.location().origin().ok()
}

/// Builds an issuer-admin-api client bound to the page's own origin (the
/// same-origin reverse proxy is expected to forward
/// `<issuer_admin_api_path>/<version>/...` to the real issuer-admin-api -
/// see `src/config.rs`, whose `issuer_admin_api_path` is passed straight
/// into the client here). Returns `None` only when the browser origin
/// cannot be determined.
fn build_client(bearer_token: Option<String>, admin_api_path: String) -> Option<IssuerAdminApiClient> {
    let origin = browser_origin()?;
    Some(IssuerAdminApiClient::new(
        reqwest::Client::new(),
        origin,
        admin_api_path,
        bearer_token,
        IdentityHubClientVersion::V1Beta,
    ))
}

fn describe_error(error: IdentityHubClientError) -> String {
    match error {
        IdentityHubClientError::Reqwest(error) => error.to_string(),
        IdentityHubClientError::Response(response) => {
            format!("the issuer-admin-api responded with {}", response.status())
        }
    }
}

/// Placeholder for the `/holders` route: an approval-workflow view over
/// EDC's issuer-admin-api holders resource, scoped to whichever
/// participant context is currently selected.
#[function_component(Holders)]
pub fn holders() -> Html {
    let (app_state, _dispatch) = use_store::<AppState>();
    let participant_context_id = app_state.selected_participant_context.clone();
    let bearer_token = app_state
        .config
        .as_ref()
        .and_then(|config| config.bearer_token.clone());
    let admin_api_path = app_state
        .config
        .as_ref()
        .map(|config| config.issuer_admin_api_path.clone())
        .unwrap_or_default();

    let active_tab = use_state(|| HoldersTab::Approved);
    let onselect = {
        let active_tab = active_tab.clone();
        Callback::from(move |tab| active_tab.set(tab))
    };

    html! {
        <PageSection>
            <Content>
                <Title level={Level::H1} size={Size::XXLarge}>{ "Holders" }</Title>
                <p>{ "Decide which DIDs this authority trusts to request verifiable credentials." }</p>
            </Content>

            if participant_context_id.is_none() {
                <EmptyState
                    title="No participant context selected"
                    icon={Icon::Users}
                    full_height=true
                >
                    <p>
                        { "Select a participant context (see Identity Bootstrap) before reviewing holders." }
                    </p>
                </EmptyState>
            } else {
                <Tabs<HoldersTab> selected={*active_tab} {onselect}>
                    <Tab<HoldersTab> index={HoldersTab::Approved} title="Approved">
                        <ApprovedHoldersPanel
                            participant_context_id={participant_context_id.clone().unwrap_or_default()}
                            bearer_token={bearer_token.clone()}
                            admin_api_path={admin_api_path.clone()}
                        />
                    </Tab<HoldersTab>>
                    <Tab<HoldersTab> index={HoldersTab::Pending} title="Pending Review">
                        <PendingHoldersPanel />
                    </Tab<HoldersTab>>
                </Tabs<HoldersTab>>
            }
        </PageSection>
    }
}

#[derive(Properties, PartialEq)]
struct ApprovedHoldersPanelProps {
    participant_context_id: String,
    bearer_token: Option<String>,
    admin_api_path: String,
}

/// The real, working half of the workflow: list/create/delete against
/// EDC's actual `Holder` records for the selected participant context.
#[function_component(ApprovedHoldersPanel)]
fn approved_holders_panel(props: &ApprovedHoldersPanelProps) -> Html {
    let load_state = use_state(|| LoadState::Loading);
    let reload_token = use_state(|| 0u32);
    let mutation_error = use_state(|| Option::<String>::None);
    let show_add_modal = use_state(|| false);
    let form_id = use_state(String::new);
    let form_did = use_state(String::new);
    let form_name = use_state(String::new);
    let submitting = use_state(|| false);

    {
        let load_state = load_state.clone();
        let participant_context_id = props.participant_context_id.clone();
        let bearer_token = props.bearer_token.clone();
        let admin_api_path = props.admin_api_path.clone();
        use_effect_with(
            (participant_context_id.clone(), *reload_token),
            move |_| {
                load_state.set(LoadState::Loading);
                let load_state = load_state.clone();
                let participant_context_id = participant_context_id.clone();
                let bearer_token = bearer_token.clone();
                let admin_api_path = admin_api_path.clone();
                spawn_local(async move {
                    let Some(client) = build_client(bearer_token, admin_api_path) else {
                        load_state.set(LoadState::Error(
                            "could not determine the page origin".to_string(),
                        ));
                        return;
                    };

                    match client
                        .query_holders(&participant_context_id, &QuerySpec::default())
                        .await
                    {
                        Ok(holders) => load_state.set(LoadState::Loaded(holders)),
                        Err(error) => load_state.set(LoadState::Error(describe_error(error))),
                    }
                });

                || ()
            },
        );
    }

    let open_add_modal = {
        let show_add_modal = show_add_modal.clone();
        let form_id = form_id.clone();
        let form_did = form_did.clone();
        let form_name = form_name.clone();
        let mutation_error = mutation_error.clone();
        Callback::from(move |_: MouseEvent| {
            form_id.set(String::new());
            form_did.set(String::new());
            form_name.set(String::new());
            mutation_error.set(None);
            show_add_modal.set(true);
        })
    };

    let close_add_modal = {
        let show_add_modal = show_add_modal.clone();
        Callback::from(move |_| show_add_modal.set(false))
    };

    let on_submit_new_holder = {
        let form_id = form_id.clone();
        let form_did = form_did.clone();
        let form_name = form_name.clone();
        let show_add_modal = show_add_modal.clone();
        let mutation_error = mutation_error.clone();
        let reload_token = reload_token.clone();
        let submitting = submitting.clone();
        let participant_context_id = props.participant_context_id.clone();
        let bearer_token = props.bearer_token.clone();
        let admin_api_path = props.admin_api_path.clone();
        Callback::from(move |_: MouseEvent| {
            let holder = HolderDto::new((*form_id).clone(), (*form_did).clone(), (*form_name).clone());
            let show_add_modal = show_add_modal.clone();
            let mutation_error = mutation_error.clone();
            let reload_token = reload_token.clone();
            let submitting = submitting.clone();
            let participant_context_id = participant_context_id.clone();
            let bearer_token = bearer_token.clone();
            let admin_api_path = admin_api_path.clone();
            let current_reload_token = *reload_token;

            submitting.set(true);
            spawn_local(async move {
                let Some(client) = build_client(bearer_token, admin_api_path) else {
                    mutation_error.set(Some("could not determine the page origin".to_string()));
                    submitting.set(false);
                    return;
                };

                match client.create_holder(&participant_context_id, &holder).await {
                    Ok(()) => {
                        mutation_error.set(None);
                        show_add_modal.set(false);
                        reload_token.set(current_reload_token + 1);
                    }
                    Err(error) => mutation_error.set(Some(describe_error(error))),
                }
                submitting.set(false);
            });
        })
    };

    let on_delete = {
        let mutation_error = mutation_error.clone();
        let reload_token = reload_token.clone();
        let participant_context_id = props.participant_context_id.clone();
        let bearer_token = props.bearer_token.clone();
        let admin_api_path = props.admin_api_path.clone();
        Callback::from(move |holder_id: String| {
            let mutation_error = mutation_error.clone();
            let reload_token = reload_token.clone();
            let participant_context_id = participant_context_id.clone();
            let bearer_token = bearer_token.clone();
            let admin_api_path = admin_api_path.clone();
            let current_reload_token = *reload_token;

            let confirmed = web_sys::window()
                .and_then(|window| {
                    window
                        .confirm_with_message(&format!(
                            "Revoke trust for holder \"{holder_id}\"? This deletes the Holder record."
                        ))
                        .ok()
                })
                .unwrap_or(true);

            if !confirmed {
                return;
            }

            spawn_local(async move {
                let Some(client) = build_client(bearer_token, admin_api_path) else {
                    mutation_error.set(Some("could not determine the page origin".to_string()));
                    return;
                };

                match client.delete_holder(&participant_context_id, &holder_id).await {
                    Ok(()) => {
                        mutation_error.set(None);
                        reload_token.set(current_reload_token + 1);
                    }
                    Err(error) => mutation_error.set(Some(describe_error(error))),
                }
            });
        })
    };

    html! {
        <>
            if let Some(message) = &*mutation_error {
                <Alert r#type={AlertType::Danger} title="Holder action failed" inline=true>
                    <p>{ message.clone() }</p>
                </Alert>
            }

            <Toolbar>
                <ToolbarContent>
                    <ToolbarItem>
                        <Button
                            label="Add holder"
                            icon={Icon::Plus}
                            variant={ButtonVariant::Primary}
                            onclick={open_add_modal}
                        />
                    </ToolbarItem>
                </ToolbarContent>
            </Toolbar>

            {
                match &*load_state {
                    LoadState::Loading => html! {
                        <Bullseye>
                            <Spinner size={SpinnerSize::Xl} aria_label="Loading holders" />
                        </Bullseye>
                    },
                    LoadState::Error(message) => html! {
                        <Alert r#type={AlertType::Danger} title="Could not load holders" inline=true>
                            <p>{ message.clone() }</p>
                        </Alert>
                    },
                    LoadState::Loaded(holders) if holders.is_empty() => html! {
                        <EmptyState title="No approved holders yet" icon={Icon::Users}>
                            <p>{ "Add a holder to trust its DID to request credentials from this authority." }</p>
                        </EmptyState>
                    },
                    LoadState::Loaded(holders) => html! {
                        <HoldersTable holders={holders.clone()} on_delete={on_delete} />
                    },
                }
            }

            if *show_add_modal {
                <Modal
                    title="Add holder"
                    variant={ModalVariant::Medium}
                    onclose={close_add_modal.clone()}
                    footer={html!(
                        <>
                            <Button
                                label={ if *submitting { "Adding..." } else { "Add" } }
                                variant={ButtonVariant::Primary}
                                disabled={*submitting}
                                onclick={on_submit_new_holder}
                            />
                            <Button
                                label="Cancel"
                                variant={ButtonVariant::Link}
                                disabled={*submitting}
                                onclick={close_add_modal.reform(|_: MouseEvent| ())}
                            />
                        </>
                    )}
                >
                    <Form>
                        <FormGroup label="Holder ID" required=true>
                            <TextInput
                                value={(*form_id).clone()}
                                required=true
                                placeholder="holder-1"
                                onchange={{
                                    let form_id = form_id.clone();
                                    Callback::from(move |value: String| form_id.set(value))
                                }}
                            />
                        </FormGroup>
                        <FormGroup label="DID" required=true>
                            <TextInput
                                value={(*form_did).clone()}
                                required=true
                                placeholder="did:web:example.com:holder-1"
                                onchange={{
                                    let form_did = form_did.clone();
                                    Callback::from(move |value: String| form_did.set(value))
                                }}
                            />
                        </FormGroup>
                        <FormGroup label="Name" required=true>
                            <TextInput
                                value={(*form_name).clone()}
                                required=true
                                placeholder="Example Holder"
                                onchange={{
                                    let form_name = form_name.clone();
                                    Callback::from(move |value: String| form_name.set(value))
                                }}
                            />
                        </FormGroup>
                    </Form>
                </Modal>
            }
        </>
    }
}

#[derive(Properties, PartialEq)]
struct HoldersTableProps {
    holders: Vec<Holder>,
    on_delete: Callback<String>,
}

#[function_component(HoldersTable)]
fn holders_table(props: &HoldersTableProps) -> Html {
    html! {
        <ComposableTable>
            <thead>
                <tr class="pf-v6-c-table__tr">
                    <th class="pf-v6-c-table__th" scope="col">{ "Holder ID" }</th>
                    <th class="pf-v6-c-table__th" scope="col">{ "DID" }</th>
                    <th class="pf-v6-c-table__th" scope="col">{ "Name" }</th>
                    <th class="pf-v6-c-table__th" scope="col">{ "Anonymous" }</th>
                    <th class="pf-v6-c-table__th" scope="col">{ "" }</th>
                </tr>
            </thead>
            <TableBody>
                { for props.holders.iter().map(|holder| {
                    let holder_id = holder.holder_id.clone();
                    let on_delete = props.on_delete.clone();
                    let onclick = Callback::from(move |_: MouseEvent| on_delete.emit(holder_id.clone()));
                    html! {
                        <TableRow key={holder.holder_id.clone()}>
                            <TableData>{ holder.holder_id.clone() }</TableData>
                            <TableData>{ holder.did.clone() }</TableData>
                            <TableData>{ holder.holder_name.clone() }</TableData>
                            <TableData>{ if holder.anonymous { "yes" } else { "no" } }</TableData>
                            <TableData action=true>
                                <Button
                                    label="Delete"
                                    icon={Icon::Trash}
                                    variant={ButtonVariant::DangerSecondary}
                                    {onclick}
                                />
                            </TableData>
                        </TableRow>
                    }
                }) }
            </TableBody>
        </ComposableTable>
    }
}

/// Not yet wired to a real data source: EDC's `Holder` API has no
/// pending/incoming-request concept, so there is nothing genuine to list
/// here. This intentionally shows no data (real or fabricated) rather than
/// simulate a workflow the backend does not support.
#[function_component(PendingHoldersPanel)]
fn pending_holders_panel() -> Html {
    html! {
        <EmptyState title="Not yet wired to a real source" icon={Icon::History} full_height=true>
            <p>
                { "EDC's issuer-admin-api Holder model has no pending/approval state - creating \
                   a holder (under \"Approved\") "}<em>{ "is" }</em>{ " the trust decision. This tab is a \
                   placeholder for a future incoming-registration-request source; it deliberately \
                   shows no data, real or fake, until one exists." }
            </p>
        </EmptyState>
    }
}

/// Component-level tests, driven via `wasm-bindgen-test` (`wasm-pack test
/// --headless --chrome`). They only assert on markup that is present on
/// the *first* render (the static shell, tab labels, the always-visible
/// "Add holder" toolbar button, the pending-tab placeholder copy) so they
/// don't depend on the async holders fetch - against a test-harness origin
/// with no real issuer-admin-api behind it - ever resolving.
// `Dispatch::global()` (yewdux) and the DOM-mounting `wasm_bindgen_test`
// helpers below only compile for `target_arch = "wasm32"` (see yewdux's own
// `#[cfg(any(doc, feature = "doctests", target_arch = "wasm32"))]` on
// `Dispatch::global`), so this whole module has to be wasm32-gated -- same
// fix as the sibling `dom_tests` module in `src/views/credentials.rs`.
// Exercise these via `wasm-pack test --headless --chrome` (or `--firefox`).
#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::*;
    use gloo_timers::future::TimeoutFuture;
    use wasm_bindgen_test::*;
    use yewdux::Dispatch;

    wasm_bindgen_test_configure!(run_in_browser);

    /// Yields back to the browser event loop once, long enough for yew's
    /// initial mount to have committed to the real DOM.
    async fn settle() {
        TimeoutFuture::new(0).await;
    }

    fn mount_in_fresh_div() -> web_sys::Element {
        let document = web_sys::window()
            .expect("window")
            .document()
            .expect("document");
        let root = document.create_element("div").expect("create_element");
        document
            .body()
            .expect("body")
            .append_child(&root)
            .expect("append_child");
        yew::Renderer::<Holders>::with_root(root.clone()).render();
        root
    }

    #[wasm_bindgen_test]
    async fn shows_guidance_when_no_participant_context_is_selected() {
        Dispatch::<AppState>::global().set(AppState {
            selected_participant_context: None,
            config: None,
        });

        let root = mount_in_fresh_div();
        settle().await;

        let text = root.text_content().unwrap_or_default();
        assert!(
            text.contains("No participant context selected"),
            "expected the empty-state guidance, got: {text}"
        );
        assert!(
            !text.contains("Approved") && !text.contains("Pending Review"),
            "tabs should not render before a participant context is selected, got: {text}"
        );

        root.remove();
    }

    #[wasm_bindgen_test]
    async fn pending_tab_is_a_clearly_labelled_placeholder_when_a_context_is_selected() {
        Dispatch::<AppState>::global().set(AppState {
            selected_participant_context: Some("participant-1".to_string()),
            config: None,
        });

        let root = mount_in_fresh_div();
        settle().await;

        let text = root.text_content().unwrap_or_default();
        assert!(
            text.contains("Add holder"),
            "expected the Approved tab's toolbar, got: {text}"
        );
        assert!(
            text.contains("Not yet wired to a real source"),
            "expected the pending-tab placeholder copy, got: {text}"
        );
        assert!(
            text.contains("EDC's issuer-admin-api Holder model has no pending/approval state"),
            "placeholder copy should explain the real API's limitation, got: {text}"
        );

        root.remove();
    }
}
