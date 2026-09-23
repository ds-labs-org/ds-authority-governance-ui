//! `/credentials` route: verifiable-credential lifecycle (list / revoke /
//! suspend / resume / status) and issuance-process monitoring, both scoped
//! to the participant context selected in `AppState`.

use std::collections::HashMap;

use edc_identity_hub_client::models::{
    CredentialStatusResponse, IssuanceProcessDto, QuerySpec, VerifiableCredentialResourceDto,
};
use edc_identity_hub_client::{
    IdentityHubClientError, IdentityHubClientVersion, IssuerAdminApiClient,
};
use patternfly_yew::prelude::*;
use yew::platform::spawn_local;
use yew::prelude::*;
use yewdux::prelude::*;

use crate::config::Config;
use crate::store::AppState;

/// Builds the issuer-admin-api client for `config`.
///
/// `endpoint` is deliberately left empty. `IssuerAdminApiClient`'s URL
/// template already hardcodes the `/api/issuer` segment - mirroring the real
/// EDC issuer-service context path, the same convention `IdentityHubClient`
/// uses for its own hardcoded `/api/identity` - which is exactly what
/// `config.issuer_admin_api_path` names as this app's same-origin proxy
/// prefix. Passing that path in as `endpoint` too would double it up into
/// `/api/issuer/api/issuer/...`. If a deployment ever needs a *different*
/// same-origin prefix than `/api/issuer`, this coupling needs revisiting on
/// the client-crate side (its URL template), not here.
fn issuer_admin_api_client(config: &Config) -> IssuerAdminApiClient {
    IssuerAdminApiClient::new(
        reqwest::Client::new(),
        String::new(),
        config.bearer_token.clone(),
        IdentityHubClientVersion::V1Beta,
    )
}

fn describe_error(error: IdentityHubClientError) -> String {
    match error {
        IdentityHubClientError::Reqwest(error) => error.to_string(),
        IdentityHubClientError::Response(response) => {
            format!("request failed: HTTP {}", response.status())
        }
    }
}

/// Maps a free-text credential/issuance-process status to a `Label` color.
/// Both APIs return free-text strings rather than a shared enum (see the
/// doc comments on `VerifiableCredentialResourceDto`/`IssuanceProcessDto` in
/// the client crate for why), so this matches case-insensitively on
/// substrings rather than an exhaustive set of known values.
fn status_color(status: &str) -> Color {
    let status = status.to_ascii_uppercase();
    if status.contains("REVOKE") || status.contains("ERROR") || status.contains("EXPIR") {
        Color::Red
    } else if status.contains("SUSPEND") {
        Color::Orange
    } else if status.contains("ISSU")
        || status.contains("ACTIVE")
        || status.contains("DELIVER")
        || status.contains("APPROV")
    {
        Color::Green
    } else {
        Color::Grey
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CredentialsTab {
    Credentials,
    IssuanceProcesses,
}

#[derive(Clone, Debug, PartialEq)]
enum Loadable<T> {
    Loading,
    Loaded(T),
    Failed(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CredentialAction {
    Revoke,
    Suspend,
    Resume,
}

impl CredentialAction {
    fn label(self) -> &'static str {
        match self {
            CredentialAction::Revoke => "Revoke",
            CredentialAction::Suspend => "Suspend",
            CredentialAction::Resume => "Resume",
        }
    }
}

/// Placeholder for the `/credentials` route (credentials & issuance).
#[function_component(Credentials)]
pub fn credentials() -> Html {
    let (app_state, _dispatch) = use_store::<AppState>();
    let participant_context_id = app_state.selected_participant_context.clone();
    let config = app_state.config.clone();

    let tab = use_state_eq(|| CredentialsTab::Credentials);
    let credentials_state: UseStateHandle<Loadable<Vec<VerifiableCredentialResourceDto>>> =
        use_state(|| Loadable::Loading);
    let issuance_state: UseStateHandle<Loadable<Vec<IssuanceProcessDto>>> =
        use_state(|| Loadable::Loading);
    let statuses: UseStateHandle<HashMap<String, CredentialStatusResponse>> =
        use_state(HashMap::new);
    let action_error: UseStateHandle<Option<String>> = use_state(|| None);
    let confirming: UseStateHandle<Option<(CredentialAction, String)>> = use_state(|| None);
    let reload = use_state(|| 0u32);

    // Loads the credential list and the issuance-process list whenever the
    // selected participant context changes, or a lifecycle action bumps
    // `reload`. Hooks must run unconditionally on every render (including
    // when no participant context is selected), so the "nothing selected"
    // early return below happens *after* every hook call.
    {
        let credentials_state = credentials_state.clone();
        let issuance_state = issuance_state.clone();
        let config = config.clone();
        use_effect_with(
            (participant_context_id.clone(), *reload),
            move |(participant_context_id, _)| {
                let participant_context_id = participant_context_id.clone();
                let config = config.clone();

                let Some(participant_context_id) = participant_context_id else {
                    credentials_state.set(Loadable::Loaded(Vec::new()));
                    issuance_state.set(Loadable::Loaded(Vec::new()));
                    return;
                };
                let Some(config) = config else {
                    let message = "configuration.json has not loaded yet".to_string();
                    credentials_state.set(Loadable::Failed(message.clone()));
                    issuance_state.set(Loadable::Failed(message));
                    return;
                };

                credentials_state.set(Loadable::Loading);
                issuance_state.set(Loadable::Loading);

                spawn_local({
                    let credentials_state = credentials_state.clone();
                    let participant_context_id = participant_context_id.clone();
                    let config = config.clone();
                    async move {
                        let client = issuer_admin_api_client(&config);
                        let result = client
                            .query_credentials(&participant_context_id, &QuerySpec::none())
                            .await;
                        credentials_state.set(match result {
                            Ok(items) => Loadable::Loaded(items),
                            Err(error) => Loadable::Failed(describe_error(error)),
                        });
                    }
                });

                spawn_local({
                    let issuance_state = issuance_state.clone();
                    async move {
                        let client = issuer_admin_api_client(&config);
                        let result = client
                            .query_issuance_processes(&participant_context_id, &QuerySpec::none())
                            .await;
                        issuance_state.set(match result {
                            Ok(items) => Loadable::Loaded(items),
                            Err(error) => Loadable::Failed(describe_error(error)),
                        });
                    }
                });
            },
        );
    }

    let Some(participant_context_id) = participant_context_id else {
        return html! {
            <PageSection>
                <Alert
                    r#type={AlertType::Info}
                    title="No participant context selected"
                    inline=true
                >
                    <p>
                        { "Select a participant context to see its issued credentials and \
                           issuance processes." }
                    </p>
                </Alert>
            </PageSection>
        };
    };

    let on_select_tab = {
        let tab = tab.clone();
        Callback::from(move |selected: CredentialsTab| tab.set(selected))
    };

    let on_check_status = {
        let statuses = statuses.clone();
        let action_error = action_error.clone();
        let config = config.clone();
        let participant_context_id = participant_context_id.clone();
        Callback::from(move |credential_id: String| {
            let Some(config) = config.clone() else {
                return;
            };
            let statuses = statuses.clone();
            let action_error = action_error.clone();
            let participant_context_id = participant_context_id.clone();
            spawn_local(async move {
                let client = issuer_admin_api_client(&config);
                match client
                    .get_credential_status(&participant_context_id, &credential_id)
                    .await
                {
                    Ok(status) => {
                        let mut next = (*statuses).clone();
                        next.insert(credential_id, status);
                        statuses.set(next);
                    }
                    Err(error) => action_error.set(Some(describe_error(error))),
                }
            });
        })
    };

    let on_request_action = {
        let confirming = confirming.clone();
        Callback::from(move |(action, credential_id): (CredentialAction, String)| {
            confirming.set(Some((action, credential_id)));
        })
    };

    let cancel_action = {
        let confirming = confirming.clone();
        Callback::from(move |_: ()| confirming.set(None))
    };

    let confirm_action = {
        let confirming = confirming.clone();
        let action_error = action_error.clone();
        let reload = reload.clone();
        let statuses = statuses.clone();
        let config = config.clone();
        let participant_context_id = participant_context_id.clone();
        Callback::from(move |_: MouseEvent| {
            let Some((action, credential_id)) = (*confirming).clone() else {
                return;
            };
            let Some(config) = config.clone() else {
                return;
            };
            confirming.set(None);
            action_error.set(None);

            let action_error = action_error.clone();
            let reload = reload.clone();
            let statuses = statuses.clone();
            let reload_value = *reload;
            let participant_context_id = participant_context_id.clone();
            spawn_local(async move {
                let client = issuer_admin_api_client(&config);
                let result = match action {
                    CredentialAction::Revoke => {
                        client
                            .revoke_credential(&participant_context_id, &credential_id)
                            .await
                    }
                    CredentialAction::Suspend => {
                        client
                            .suspend_credential(&participant_context_id, &credential_id)
                            .await
                    }
                    CredentialAction::Resume => {
                        client
                            .resume_credential(&participant_context_id, &credential_id)
                            .await
                    }
                };

                match result {
                    Ok(()) => {
                        // Drop any cached status for this credential so the
                        // table shows "Check status" again until it's
                        // rechecked, and bump `reload` to refresh the list.
                        let mut next = (*statuses).clone();
                        next.remove(&credential_id);
                        statuses.set(next);
                        reload.set(reload_value + 1);
                    }
                    Err(error) => action_error.set(Some(describe_error(error))),
                }
            });
        })
    };

    html! {
        <PageSection>
            <Title level={Level::H2} size={Size::XLarge}>{ "Credentials & Issuance" }</Title>
            <p>{ format!("Participant context: {participant_context_id}") }</p>

            if let Some(message) = &*action_error {
                <Alert
                    r#type={AlertType::Danger}
                    title="Action failed"
                    inline=true
                    onclose={
                        let action_error = action_error.clone();
                        Callback::from(move |_| action_error.set(None))
                    }
                >
                    <p>{ message.clone() }</p>
                </Alert>
            }

            <Tabs<CredentialsTab> selected={*tab} onselect={on_select_tab}>
                <Tab<CredentialsTab> index={CredentialsTab::Credentials} title="Credentials">
                    { credentials_panel(&credentials_state, &statuses, &on_check_status, &on_request_action) }
                </Tab<CredentialsTab>>
                <Tab<CredentialsTab> index={CredentialsTab::IssuanceProcesses} title="Issuance Processes">
                    { issuance_processes_panel(&issuance_state) }
                </Tab<CredentialsTab>>
            </Tabs<CredentialsTab>>

            if let Some((action, credential_id)) = (*confirming).clone() {
                <Modal
                    title={format!("{} credential", action.label())}
                    variant={ModalVariant::Small}
                    onclose={
                        let cancel_action = cancel_action.clone();
                        Callback::from(move |_| cancel_action.emit(()))
                    }
                    footer={html!(
                        <>
                            <Button
                                label={action.label()}
                                variant={
                                    if action == CredentialAction::Revoke {
                                        ButtonVariant::Danger
                                    } else {
                                        ButtonVariant::Primary
                                    }
                                }
                                onclick={confirm_action.clone()}
                            />
                            <Button
                                label="Cancel"
                                variant={ButtonVariant::Link}
                                onclick={
                                    let cancel_action = cancel_action.clone();
                                    Callback::from(move |_| cancel_action.emit(()))
                                }
                            />
                        </>
                    )}
                >
                    <p>
                        { format!(
                            "{} credential {credential_id}? This cannot be undone from this console.",
                            action.label()
                        ) }
                    </p>
                </Modal>
            }
        </PageSection>
    }
}

fn credentials_panel(
    state: &Loadable<Vec<VerifiableCredentialResourceDto>>,
    statuses: &HashMap<String, CredentialStatusResponse>,
    on_check_status: &Callback<String>,
    on_request_action: &Callback<(CredentialAction, String)>,
) -> Html {
    match state {
        Loadable::Loading => html! {
            <Bullseye><Spinner aria_label="Loading credentials" /></Bullseye>
        },
        Loadable::Failed(message) => html! {
            <Alert r#type={AlertType::Danger} title="Could not load credentials" inline=true>
                <p>{ message.clone() }</p>
            </Alert>
        },
        Loadable::Loaded(items) if items.is_empty() => html! {
            <Alert r#type={AlertType::Info} title="No credentials issued yet" inline=true>
                <p>{ "This participant context has no verifiable credentials on record." }</p>
            </Alert>
        },
        Loadable::Loaded(items) => html! {
            <ComposableTable>
                <thead>
                    <tr class="pf-v6-c-table__tr">
                        <th class="pf-v6-c-table__th">{ "ID" }</th>
                        <th class="pf-v6-c-table__th">{ "Format" }</th>
                        <th class="pf-v6-c-table__th">{ "Status" }</th>
                        <th class="pf-v6-c-table__th">{ "Actions" }</th>
                    </tr>
                </thead>
                <TableBody>
                    { for items.iter().map(|item| {
                        credential_row(item, statuses.get(&item.id), on_check_status, on_request_action)
                    }) }
                </TableBody>
            </ComposableTable>
        },
    }
}

fn credential_row(
    item: &VerifiableCredentialResourceDto,
    status: Option<&CredentialStatusResponse>,
    on_check_status: &Callback<String>,
    on_request_action: &Callback<(CredentialAction, String)>,
) -> Html {
    let credential_id = item.id.clone();

    let status_cell = match status {
        Some(status) => html! {
            <Label label={status.status.clone()} color={status_color(&status.status)} />
        },
        None => {
            let credential_id = credential_id.clone();
            let on_check_status = on_check_status.clone();
            html! {
                <Button
                    label="Check status"
                    variant={ButtonVariant::Link}
                    onclick={Callback::from(move |_| on_check_status.emit(credential_id.clone()))}
                />
            }
        }
    };

    let action_button = |action: CredentialAction| {
        let on_request_action = on_request_action.clone();
        let credential_id = credential_id.clone();
        let variant = if action == CredentialAction::Revoke {
            ButtonVariant::DangerSecondary
        } else {
            ButtonVariant::Secondary
        };
        html! {
            <Button
                label={action.label()}
                {variant}
                onclick={Callback::from(move |_| on_request_action.emit((action, credential_id.clone())))}
            />
        }
    };

    html! {
        <TableRow>
            <TableData>{ item.id.clone() }</TableData>
            <TableData>{ item.format.clone() }</TableData>
            <TableData>{ status_cell }</TableData>
            <TableData>
                <div style="display:flex;gap:0.5rem;flex-wrap:wrap;">
                    { action_button(CredentialAction::Revoke) }
                    { action_button(CredentialAction::Suspend) }
                    { action_button(CredentialAction::Resume) }
                </div>
            </TableData>
        </TableRow>
    }
}

fn issuance_processes_panel(state: &Loadable<Vec<IssuanceProcessDto>>) -> Html {
    match state {
        Loadable::Loading => html! {
            <Bullseye><Spinner aria_label="Loading issuance processes" /></Bullseye>
        },
        Loadable::Failed(message) => html! {
            <Alert r#type={AlertType::Danger} title="Could not load issuance processes" inline=true>
                <p>{ message.clone() }</p>
            </Alert>
        },
        Loadable::Loaded(items) if items.is_empty() => html! {
            <Alert r#type={AlertType::Info} title="No issuance processes in flight" inline=true>
                <p>
                    { "This participant context has no active or historical issuance \
                       processes on record." }
                </p>
            </Alert>
        },
        Loadable::Loaded(items) => html! {
            <ComposableTable>
                <thead>
                    <tr class="pf-v6-c-table__tr">
                        <th class="pf-v6-c-table__th">{ "ID" }</th>
                        <th class="pf-v6-c-table__th">{ "Holder" }</th>
                        <th class="pf-v6-c-table__th">{ "State" }</th>
                        <th class="pf-v6-c-table__th">{ "Created" }</th>
                        <th class="pf-v6-c-table__th">{ "Updated" }</th>
                    </tr>
                </thead>
                <TableBody>
                    { for items.iter().map(issuance_process_row) }
                </TableBody>
            </ComposableTable>
        },
    }
}

fn issuance_process_row(process: &IssuanceProcessDto) -> Html {
    html! {
        <TableRow>
            <TableData>{ process.id.clone() }</TableData>
            <TableData>{ process.holder_id.clone() }</TableData>
            <TableData>
                <Label label={process.state.clone()} color={status_color(&process.state)} />
            </TableData>
            <TableData>{ process.created_at.to_rfc3339() }</TableData>
            <TableData>{ process.updated_at.to_rfc3339() }</TableData>
        </TableRow>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_color_maps_revoked_to_red() {
        assert_eq!(status_color("REVOKED"), Color::Red);
        assert_eq!(status_color("revoked"), Color::Red);
    }

    #[test]
    fn status_color_maps_suspended_to_orange() {
        assert_eq!(status_color("SUSPENDED"), Color::Orange);
    }

    #[test]
    fn status_color_maps_active_states_to_green() {
        assert_eq!(status_color("active"), Color::Green);
        assert_eq!(status_color("ISSUED"), Color::Green);
        assert_eq!(status_color("DELIVERED"), Color::Green);
        assert_eq!(status_color("APPROVED"), Color::Green);
    }

    #[test]
    fn status_color_defaults_unknown_statuses_to_grey() {
        assert_eq!(status_color("SOMETHING_ELSE"), Color::Grey);
    }

    #[test]
    fn credential_action_labels_are_human_readable() {
        assert_eq!(CredentialAction::Revoke.label(), "Revoke");
        assert_eq!(CredentialAction::Suspend.label(), "Suspend");
        assert_eq!(CredentialAction::Resume.label(), "Resume");
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod dom_tests {
    use super::*;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    /// Smoke test for the "no participant context selected" branch - the
    /// default `AppState` (nothing dispatched) already has
    /// `selected_participant_context: None`, so mounting `<Credentials />`
    /// against a fresh yewdux store must hit that early-return branch
    /// without making any network call.
    #[wasm_bindgen_test]
    async fn prompts_for_a_participant_context_when_none_is_selected() {
        let document = web_sys::window().unwrap().document().unwrap();
        let root = document.create_element("div").unwrap();
        document.body().unwrap().append_child(&root).unwrap();

        yew::Renderer::<Credentials>::with_root(root.clone()).render();

        // Yield a tick so Yew's scheduler flushes the initial render.
        gloo_timers::future::TimeoutFuture::new(0).await;

        let text = root.text_content().unwrap_or_default();
        assert!(
            text.contains("Select a participant context"),
            "expected the no-selection prompt, got: {text}"
        );
    }
}
