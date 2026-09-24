//! `/credential-definitions` route: lists, creates, edits, and deletes the credential
//! definitions the currently-selected participant context is willing to issue.
//!
//! This is a live-authoring surface (unlike the read-only/git-governed views elsewhere
//! in this app), so every mutation round-trips through the issuer-admin-api immediately
//! via `edc_identity_hub_client::IssuerAdminApiClient` and then re-fetches the list.

use edc_identity_hub_client::models::{CredentialDefinition, CredentialDefinitionDto, QuerySpec};
use edc_identity_hub_client::{IdentityHubClientError, IdentityHubClientVersion, IssuerAdminApiClient};
use patternfly_yew::prelude::*;
use yew::platform::spawn_local;
use yew::prelude::*;
use yewdux::prelude::*;

use crate::config::Config;
use crate::store::AppState;

/// Placeholder-replacing route view. Kept to a single `PageSection` (no nested `Page`)
/// per this app's shell convention - `main.rs` already supplies the masthead/nav chrome.
#[function_component(CredentialDefinitions)]
pub fn credential_definitions() -> Html {
    html! {
        <PageSection>
            <BackdropViewer>
                <CredentialDefinitionsContent />
            </BackdropViewer>
        </PageSection>
    }
}

/// Where the credential-definitions list currently stands.
#[derive(Clone, PartialEq)]
enum ListState {
    Loading,
    Loaded(Vec<CredentialDefinition>),
    Error(String),
}

/// Which modal, if any, is currently open over the list.
#[derive(Clone, PartialEq)]
enum ModalState {
    None,
    Create,
    Edit(CredentialDefinition),
    ConfirmDelete(CredentialDefinition),
}

fn current_origin() -> Option<String> {
    web_sys::window()?.location().origin().ok()
}

/// Builds the issuer-admin-api client for this page's own origin.
///
/// The client's methods build URLs as
/// `{endpoint}{admin_api_path}/{version}/participants/...`, so `endpoint` here is just
/// the page's origin and `admin_api_path` is `config.issuer_admin_api_path`: this app is
/// served same-origin behind a reverse proxy that forwards
/// `<issuer_admin_api_path>/...` requests to the real issuer-admin-api unchanged (see
/// `Config`'s doc comment on `issuer_admin_api_path`, which names that same proxied
/// prefix).
///
/// No API key is passed to the client: issuer-admin-api's own `x-api-key`
/// auth is injected server-side by apisix, never by this app (see
/// `Config`'s doc comment). The browser's same-origin OIDC session cookie,
/// when present, is sent automatically on top of this regardless.
fn build_client(config: &Config) -> Option<IssuerAdminApiClient> {
    let origin = current_origin()?;
    Some(IssuerAdminApiClient::new(
        reqwest::Client::new(),
        origin,
        config.issuer_admin_api_path.clone(),
        None,
        IdentityHubClientVersion::V1Beta,
    ))
}

async fn describe_error(error: IdentityHubClientError) -> String {
    match error {
        IdentityHubClientError::Reqwest(error) => error.to_string(),
        IdentityHubClientError::Response(response) => {
            let status = response.status();
            match response.text().await {
                Ok(body) if !body.trim().is_empty() => format!("HTTP {status}: {body}"),
                _ => format!("HTTP {status}"),
            }
        }
    }
}

#[function_component(CredentialDefinitionsContent)]
fn credential_definitions_content() -> Html {
    let (app_state, _dispatch) = use_store::<AppState>();
    let participant_context_id = app_state.selected_participant_context.clone();
    let config = app_state.config.clone();

    let list_state = use_state(|| ListState::Loading);
    let modal_state = use_state(|| ModalState::None);
    // Bumped after every create/update/delete to trigger a re-fetch.
    let reload = use_state(|| 0u32);

    {
        let list_state = list_state.clone();
        let participant_context_id = participant_context_id.clone();
        let config = config.clone();
        use_effect_with(
            (participant_context_id.clone(), *reload),
            move |(participant_context_id, _)| {
                let Some(participant_context_id) = participant_context_id.clone() else {
                    return;
                };
                let Some(config) = config else {
                    list_state.set(ListState::Error(
                        "Configuration has not finished loading.".to_string(),
                    ));
                    return;
                };
                list_state.set(ListState::Loading);
                spawn_local(async move {
                    let Some(client) = build_client(&config) else {
                        list_state.set(ListState::Error(
                            "Could not determine the page origin.".to_string(),
                        ));
                        return;
                    };
                    match client
                        .query_credential_definitions(&participant_context_id, &QuerySpec::default())
                        .await
                    {
                        Ok(definitions) => list_state.set(ListState::Loaded(definitions)),
                        Err(error) => list_state.set(ListState::Error(describe_error(error).await)),
                    }
                });
            },
        );
    }

    let Some(participant_context_id) = participant_context_id else {
        return html! {
            <EmptyState title="No participant context selected">
                <p>{ "Select a participant context to view and author its credential definitions." }</p>
            </EmptyState>
        };
    };

    let Some(config) = config else {
        return html! {
            <Alert r#type={AlertType::Danger} title="Configuration has not finished loading." inline=true />
        };
    };

    let open_create = {
        let modal_state = modal_state.clone();
        Callback::from(move |_: MouseEvent| modal_state.set(ModalState::Create))
    };

    let close_modal = {
        let modal_state = modal_state.clone();
        Callback::from(move |()| modal_state.set(ModalState::None))
    };

    let on_saved = {
        let modal_state = modal_state.clone();
        let reload = reload.clone();
        Callback::from(move |()| {
            modal_state.set(ModalState::None);
            reload.set(*reload + 1);
        })
    };

    let request_delete = {
        let modal_state = modal_state.clone();
        Callback::from(move |definition: CredentialDefinition| {
            modal_state.set(ModalState::ConfirmDelete(definition));
        })
    };

    let request_edit = {
        let modal_state = modal_state.clone();
        Callback::from(move |definition: CredentialDefinition| {
            modal_state.set(ModalState::Edit(definition));
        })
    };

    html! {
        <>
            <Toolbar>
                <ToolbarContent>
                    <ToolbarItem>
                        <Button
                            label="Create credential definition"
                            variant={ButtonVariant::Primary}
                            onclick={open_create}
                        />
                    </ToolbarItem>
                </ToolbarContent>
            </Toolbar>

            { match &*list_state {
                ListState::Loading => html! {
                    <Bullseye><Spinner aria_label="Loading credential definitions" /></Bullseye>
                },
                ListState::Error(message) => html! {
                    <Alert r#type={AlertType::Danger} title="Could not load credential definitions" inline=true>
                        <p>{ message.clone() }</p>
                    </Alert>
                },
                ListState::Loaded(definitions) if definitions.is_empty() => html! {
                    <EmptyState title="No credential definitions yet">
                        <p>{ "This authority isn't configured to issue any credential types yet." }</p>
                    </EmptyState>
                },
                ListState::Loaded(definitions) => html! {
                    <CredentialDefinitionsTable
                        definitions={definitions.clone()}
                        onedit={request_edit}
                        ondelete={request_delete}
                    />
                },
            } }

            { match &*modal_state {
                ModalState::None => html!(),
                ModalState::Create => html! {
                    <CredentialDefinitionModal
                        title="Create credential definition"
                        submit_label="Create"
                        initial={None}
                        participant_context_id={participant_context_id.clone()}
                        config={config.clone()}
                        onsaved={on_saved.clone()}
                        onclose={close_modal.clone()}
                    />
                },
                ModalState::Edit(definition) => html! {
                    <CredentialDefinitionModal
                        title="Edit credential definition"
                        submit_label="Save"
                        initial={Some(definition.clone())}
                        participant_context_id={participant_context_id.clone()}
                        config={config.clone()}
                        onsaved={on_saved.clone()}
                        onclose={close_modal.clone()}
                    />
                },
                ModalState::ConfirmDelete(definition) => html! {
                    <DeleteConfirmModal
                        definition={definition.clone()}
                        participant_context_id={participant_context_id.clone()}
                        config={config.clone()}
                        ondeleted={on_saved.clone()}
                        onclose={close_modal.clone()}
                    />
                },
            } }
        </>
    }
}

#[derive(Properties, PartialEq)]
struct CredentialDefinitionsTableProps {
    definitions: Vec<CredentialDefinition>,
    onedit: Callback<CredentialDefinition>,
    ondelete: Callback<CredentialDefinition>,
}

/// Plain `<table>` styled with PatternFly's own table classes, in the same spirit as
/// this app's nav links (`main.rs` reaches for raw `pf-v6-c-nav__*` classes rather than
/// a heavier component there too) - `patternfly-yew`'s generic `Table<M>` needs a
/// `TableEntryRenderer` impl per row type, which is more machinery than a 5-column,
/// non-sortable, non-paginated list justifies here.
#[function_component(CredentialDefinitionsTable)]
fn credential_definitions_table(props: &CredentialDefinitionsTableProps) -> Html {
    html! {
        <table class="pf-v6-c-table" role="grid" aria-label="Credential definitions">
            <thead>
                <tr role="row">
                    <th role="columnheader">{ "Credential type" }</th>
                    <th role="columnheader">{ "Format" }</th>
                    <th role="columnheader">{ "Validity (s)" }</th>
                    <th role="columnheader">{ "Attestations" }</th>
                    <th role="columnheader"><span class="pf-v6-screen-reader">{ "Actions" }</span></th>
                </tr>
            </thead>
            <tbody>
                { for props.definitions.iter().cloned().map(|definition| {
                    let onedit = props.onedit.clone();
                    let ondelete = props.ondelete.clone();
                    let edit_definition = definition.clone();
                    let delete_definition = definition.clone();
                    html! {
                        <tr role="row" key={definition.id.clone()}>
                            <td role="cell" data-label="Credential type">{ definition.credential_type.clone() }</td>
                            <td role="cell" data-label="Format">{ definition.format.clone() }</td>
                            <td role="cell" data-label="Validity (s)">{ definition.validity }</td>
                            <td role="cell" data-label="Attestations">{ definition.attestations.join(", ") }</td>
                            <td role="cell" data-label="Actions">
                                <Button
                                    label="Edit"
                                    variant={ButtonVariant::Secondary}
                                    onclick={Callback::from(move |_| onedit.emit(edit_definition.clone()))}
                                />
                                { " " }
                                <Button
                                    label="Delete"
                                    variant={ButtonVariant::Danger}
                                    onclick={Callback::from(move |_| ondelete.emit(delete_definition.clone()))}
                                />
                            </td>
                        </tr>
                    }
                }) }
            </tbody>
        </table>
    }
}

/// The editable fields of a credential definition, held as plain `String`s while the
/// form is open so every input can be a simple, always-valid `TextInput`/`TextArea`;
/// validated and converted to a `CredentialDefinitionDto` only on submit.
#[derive(Clone, PartialEq)]
struct FormValues {
    id: Option<String>,
    credential_type: String,
    format: String,
    json_schema: String,
    json_schema_url: String,
    validity: String,
    attestations: String,
}

impl FormValues {
    fn empty() -> Self {
        Self {
            id: None,
            credential_type: String::new(),
            format: String::new(),
            json_schema: String::new(),
            json_schema_url: String::new(),
            validity: String::new(),
            attestations: String::new(),
        }
    }

    fn from_definition(definition: &CredentialDefinition) -> Self {
        Self {
            id: Some(definition.id.clone()),
            credential_type: definition.credential_type.clone(),
            format: definition.format.clone(),
            json_schema: definition.json_schema.clone().unwrap_or_default(),
            json_schema_url: definition.json_schema_url.clone().unwrap_or_default(),
            validity: definition.validity.to_string(),
            attestations: definition.attestations.join(", "),
        }
    }

    /// Validates and assembles the DTO the issuer-admin-api expects, mirroring the real
    /// controller's own constraints: `credentialType` is required, `validity` must be a
    /// whole number of seconds, and exactly one of `jsonSchema` / `jsonSchemaUrl` must
    /// be given (the domain object's builder XOR-requires them).
    fn to_dto(&self) -> Result<CredentialDefinitionDto, String> {
        let credential_type = self.credential_type.trim();
        if credential_type.is_empty() {
            return Err("Credential type is required.".to_string());
        }

        let validity: i64 = self
            .validity
            .trim()
            .parse()
            .map_err(|_| "Validity must be a whole number of seconds.".to_string())?;

        let json_schema = non_empty(&self.json_schema);
        let json_schema_url = non_empty(&self.json_schema_url);
        match (&json_schema, &json_schema_url) {
            (None, None) => {
                return Err("Provide either a JSON schema or a JSON schema URL.".to_string());
            }
            (Some(_), Some(_)) => {
                return Err(
                    "Provide only one of JSON schema or JSON schema URL, not both.".to_string(),
                );
            }
            _ => {}
        }

        let attestations = self
            .attestations
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .collect();

        Ok(CredentialDefinitionDto::new(
            self.id.clone(),
            credential_type.to_string(),
            non_empty(&self.format),
            json_schema,
            json_schema_url,
            validity,
            attestations,
            Vec::new(),
            Vec::new(),
        ))
    }
}

fn non_empty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[derive(Properties, PartialEq)]
struct CredentialDefinitionModalProps {
    title: AttrValue,
    submit_label: AttrValue,
    initial: Option<CredentialDefinition>,
    participant_context_id: String,
    config: Config,
    onsaved: Callback<()>,
    onclose: Callback<()>,
}

/// Create/edit modal, shared by both flows: `initial` being `None` means create.
#[function_component(CredentialDefinitionModal)]
fn credential_definition_modal(props: &CredentialDefinitionModalProps) -> Html {
    let initial = use_memo(props.initial.clone(), |initial| match initial {
        Some(definition) => FormValues::from_definition(definition),
        None => FormValues::empty(),
    });

    let credential_type = use_state(|| initial.credential_type.clone());
    let format = use_state(|| initial.format.clone());
    let json_schema = use_state(|| initial.json_schema.clone());
    let json_schema_url = use_state(|| initial.json_schema_url.clone());
    let validity = use_state(|| initial.validity.clone());
    let attestations = use_state(|| initial.attestations.clone());
    let error = use_state(|| None::<String>);
    let saving = use_state(|| false);

    let onsubmit = {
        let id = initial.id.clone();
        let credential_type = credential_type.clone();
        let format = format.clone();
        let json_schema = json_schema.clone();
        let json_schema_url = json_schema_url.clone();
        let validity = validity.clone();
        let attestations = attestations.clone();
        let error = error.clone();
        let saving = saving.clone();
        let participant_context_id = props.participant_context_id.clone();
        let config = props.config.clone();
        let onsaved = props.onsaved.clone();
        let is_edit = id.is_some();

        Callback::from(move |event: SubmitEvent| {
            event.prevent_default();

            let values = FormValues {
                id: id.clone(),
                credential_type: (*credential_type).clone(),
                format: (*format).clone(),
                json_schema: (*json_schema).clone(),
                json_schema_url: (*json_schema_url).clone(),
                validity: (*validity).clone(),
                attestations: (*attestations).clone(),
            };

            let dto = match values.to_dto() {
                Ok(dto) => dto,
                Err(message) => {
                    error.set(Some(message));
                    return;
                }
            };

            error.set(None);
            saving.set(true);

            let participant_context_id = participant_context_id.clone();
            let config = config.clone();
            let onsaved = onsaved.clone();
            let error = error.clone();
            let saving = saving.clone();

            spawn_local(async move {
                let Some(client) = build_client(&config) else {
                    error.set(Some("Could not determine the page origin.".to_string()));
                    saving.set(false);
                    return;
                };

                let result = if is_edit {
                    client
                        .update_credential_definition(&participant_context_id, &dto)
                        .await
                } else {
                    client
                        .create_credential_definition(&participant_context_id, &dto)
                        .await
                };

                match result {
                    Ok(()) => onsaved.emit(()),
                    Err(err) => {
                        error.set(Some(describe_error(err).await));
                        saving.set(false);
                    }
                }
            });
        })
    };

    let footer = html! {
        <>
            <Button
                r#type={ButtonType::Submit}
                form="credential-definition-form"
                label={props.submit_label.to_string()}
                variant={ButtonVariant::Primary}
                loading={*saving}
                disabled={*saving}
            />
            <Button
                label="Cancel"
                variant={ButtonVariant::Link}
                onclick={props.onclose.reform(|_: MouseEvent| ())}
                disabled={*saving}
            />
        </>
    };

    html! {
        <Bullseye>
            <Modal
                title={props.title.to_string()}
                variant={ModalVariant::Medium}
                onclose={props.onclose.clone()}
                footer={Some(footer)}
            >
                <Form id="credential-definition-form" onsubmit={onsubmit}>
                    if let Some(message) = &*error {
                        <Alert r#type={AlertType::Danger} title={message.clone()} inline=true />
                    }
                    <FormGroup label="Credential type" required=true>
                        <TextInput
                            required=true
                            value={(*credential_type).clone()}
                            onchange={{
                                let credential_type = credential_type.clone();
                                move |value: String| credential_type.set(value)
                            }}
                        />
                    </FormGroup>
                    <FormGroup label="Format">
                        <TextInput
                            placeholder="vc1_0_jwt"
                            value={(*format).clone()}
                            onchange={{
                                let format = format.clone();
                                move |value: String| format.set(value)
                            }}
                        />
                    </FormGroup>
                    <FormGroup
                        label="JSON schema (inline)"
                        helper_text={FormHelperText::from("Provide this or a JSON schema URL below, not both.")}
                    >
                        <TextArea
                            rows={6}
                            value={(*json_schema).clone()}
                            onchange={{
                                let json_schema = json_schema.clone();
                                move |value: String| json_schema.set(value)
                            }}
                        />
                    </FormGroup>
                    <FormGroup label="JSON schema URL">
                        <TextInput
                            value={(*json_schema_url).clone()}
                            onchange={{
                                let json_schema_url = json_schema_url.clone();
                                move |value: String| json_schema_url.set(value)
                            }}
                        />
                    </FormGroup>
                    <FormGroup label="Validity (seconds)" required=true>
                        <TextInput
                            required=true
                            r#type={TextInputType::Number}
                            value={(*validity).clone()}
                            onchange={{
                                let validity = validity.clone();
                                move |value: String| validity.set(value)
                            }}
                        />
                    </FormGroup>
                    <FormGroup
                        label="Attestations"
                        helper_text={FormHelperText::from("Comma-separated attestation identifiers.")}
                    >
                        <TextInput
                            value={(*attestations).clone()}
                            onchange={{
                                let attestations = attestations.clone();
                                move |value: String| attestations.set(value)
                            }}
                        />
                    </FormGroup>
                </Form>
            </Modal>
        </Bullseye>
    }
}

#[derive(Properties, PartialEq)]
struct DeleteConfirmModalProps {
    definition: CredentialDefinition,
    participant_context_id: String,
    config: Config,
    ondeleted: Callback<()>,
    onclose: Callback<()>,
}

#[function_component(DeleteConfirmModal)]
fn delete_confirm_modal(props: &DeleteConfirmModalProps) -> Html {
    let error = use_state(|| None::<String>);
    let deleting = use_state(|| false);

    let ondelete = {
        let definition_id = props.definition.id.clone();
        let participant_context_id = props.participant_context_id.clone();
        let config = props.config.clone();
        let ondeleted = props.ondeleted.clone();
        let error = error.clone();
        let deleting = deleting.clone();

        Callback::from(move |_: MouseEvent| {
            error.set(None);
            deleting.set(true);

            let definition_id = definition_id.clone();
            let participant_context_id = participant_context_id.clone();
            let config = config.clone();
            let ondeleted = ondeleted.clone();
            let error = error.clone();
            let deleting = deleting.clone();

            spawn_local(async move {
                let Some(client) = build_client(&config) else {
                    error.set(Some("Could not determine the page origin.".to_string()));
                    deleting.set(false);
                    return;
                };

                match client
                    .delete_credential_definition_by_id(&participant_context_id, &definition_id)
                    .await
                {
                    Ok(()) => ondeleted.emit(()),
                    Err(err) => {
                        error.set(Some(describe_error(err).await));
                        deleting.set(false);
                    }
                }
            });
        })
    };

    let footer = html! {
        <>
            <Button
                label="Delete"
                variant={ButtonVariant::Danger}
                onclick={ondelete}
                loading={*deleting}
                disabled={*deleting}
            />
            <Button
                label="Cancel"
                variant={ButtonVariant::Link}
                onclick={props.onclose.reform(|_: MouseEvent| ())}
                disabled={*deleting}
            />
        </>
    };

    html! {
        <Bullseye>
            <Modal
                title="Delete credential definition?"
                variant={ModalVariant::Small}
                onclose={props.onclose.clone()}
                footer={Some(footer)}
            >
                if let Some(message) = &*error {
                    <Alert r#type={AlertType::Danger} title={message.clone()} inline=true />
                }
                <p>
                    { "This permanently removes \"" }
                    { props.definition.credential_type.clone() }
                    { "\" from the definitions this authority will issue." }
                </p>
            </Modal>
        </Bullseye>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;
    use yewdux::Dispatch;

    wasm_bindgen_test_configure!(run_in_browser);

    fn definition() -> CredentialDefinition {
        CredentialDefinition {
            id: "cred-def-1".to_string(),
            participant_context_id: "participant-1".to_string(),
            credential_type: "MembershipCredential".to_string(),
            format: "VC1_0_JWT".to_string(),
            json_schema: Some("{\"type\":\"object\"}".to_string()),
            json_schema_url: None,
            validity: 3600,
            attestations: vec!["attestation-1".to_string()],
            additional_context: vec![],
            rules: vec![],
            mappings: vec![],
        }
    }

    #[test]
    fn to_dto_requires_credential_type() {
        let mut values = FormValues::empty();
        values.validity = "3600".to_string();
        values.json_schema = "{}".to_string();

        assert_eq!(
            values.to_dto().unwrap_err(),
            "Credential type is required."
        );
    }

    #[test]
    fn to_dto_requires_a_numeric_validity() {
        let mut values = FormValues::empty();
        values.credential_type = "MembershipCredential".to_string();
        values.json_schema = "{}".to_string();
        values.validity = "not-a-number".to_string();

        assert_eq!(
            values.to_dto().unwrap_err(),
            "Validity must be a whole number of seconds."
        );
    }

    #[test]
    fn to_dto_requires_exactly_one_schema_source() {
        let mut values = FormValues::empty();
        values.credential_type = "MembershipCredential".to_string();
        values.validity = "3600".to_string();

        assert_eq!(
            values.to_dto().unwrap_err(),
            "Provide either a JSON schema or a JSON schema URL."
        );

        values.json_schema = "{}".to_string();
        values.json_schema_url = "https://example.com/schema.json".to_string();

        assert_eq!(
            values.to_dto().unwrap_err(),
            "Provide only one of JSON schema or JSON schema URL, not both."
        );
    }

    #[test]
    fn to_dto_builds_a_valid_dto_from_valid_input() {
        let mut values = FormValues::empty();
        values.credential_type = " MembershipCredential ".to_string();
        values.format = "vc1_0_jwt".to_string();
        values.json_schema = "{\"type\":\"object\"}".to_string();
        values.validity = "3600".to_string();
        values.attestations = "att-1, att-2 ,, ".to_string();

        let dto = values.to_dto().expect("valid input should produce a DTO");

        assert_eq!(dto.credential_type, "MembershipCredential");
        assert_eq!(dto.format, Some("vc1_0_jwt".to_string()));
        assert_eq!(dto.validity, 3600);
        assert_eq!(
            dto.attestations,
            vec!["att-1".to_string(), "att-2".to_string()]
        );
    }

    #[test]
    fn from_definition_round_trips_the_domain_object_into_editable_form_values() {
        let values = FormValues::from_definition(&definition());

        assert_eq!(values.id, Some("cred-def-1".to_string()));
        assert_eq!(values.credential_type, "MembershipCredential");
        assert_eq!(values.json_schema, "{\"type\":\"object\"}");
        assert_eq!(values.json_schema_url, "");
        assert_eq!(values.validity, "3600");
        assert_eq!(values.attestations, "attestation-1");
    }

    /// Component-level test: mounts the real view into a detached DOM node and reads
    /// its rendered text back out. Exercised via `wasm-pack test --headless --chrome`
    /// (or `--firefox`), never under plain `cargo test` -- gated to wasm32 because it
    /// calls `Dispatch::global()`, which yewdux only provides for that target (same
    /// reasoning `views::holders`'s own DOM tests are gated this way; this one wasn't,
    /// which broke plain native `cargo test` outright rather than just skipping the
    /// test -- a real compile error, not a benign no-op).
    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    async fn credential_definitions_prompts_when_no_participant_context_is_selected() {
        // `AppState` is a yewdux global, shared across every test in this
        // binary -- reset it explicitly rather than relying on whatever a
        // previous test in the run happened to leave it as (same fix as
        // `views::holders`'s own tests already apply; this test just never
        // needed it before an unrelated dependency-graph change reordered
        // wasm-bindgen-test's registration and exposed the gap).
        Dispatch::<AppState>::global().set(AppState {
            selected_participant_context: None,
            config: None,
            user: None,
        });

        let document = web_sys::window()
            .expect("a window")
            .document()
            .expect("a document");
        let root = document
            .create_element("div")
            .expect("a detached container element");
        document
            .body()
            .expect("a document body")
            .append_child(&root)
            .expect("attaching the container to the body");

        yew::Renderer::<CredentialDefinitions>::with_root(root.clone()).render();

        // The initial render is scheduled on yew's own microtask queue rather than
        // happening synchronously inside `render()`, so give it one tick to flush
        // before inspecting the DOM.
        yew::platform::time::sleep(std::time::Duration::from_millis(0)).await;

        let text = root.text_content().unwrap_or_default();
        assert!(
            text.contains("Select a participant context"),
            "expected the empty-state prompt in rendered output, got: {text:?}"
        );

        root.remove();
    }
}
