//! `/credential-definitions`: list, create, edit and delete the hub's
//! credential definitions.

use patternfly_yew::prelude::*;
use yew::platform::spawn_local;
use yew::prelude::*;

use crate::api::{Api, CredentialDefinition, QuerySpec, client};
use crate::components::*;
use crate::format::{duration, timestamp_or};
use crate::hooks::{Load, use_config, use_load};
use crate::validation::{DefinitionErrors, validate_definition};

const TABLE: &str = "Credential definitions";
const COLUMNS: [&str; 6] = [
    "ID",
    "Credential type",
    "Format",
    "Validity (seconds)",
    "Created",
    "",
];

#[derive(Clone, PartialEq)]
enum Open {
    None,
    Create,
    Edit(CredentialDefinition),
    Delete(String),
}

#[function_component(CredentialDefinitions)]
pub fn credential_definitions() -> Html {
    let config = use_config();
    let (definitions, reload) = use_load(|config| async move {
        let request = Api::new(&config)
            .query_credential_definitions(&QuerySpec::all_newest_first("createdAt"));
        client::send_json::<Vec<CredentialDefinition>>(&config, &request).await
    });
    let open = use_state(|| Open::None);
    let feedback = use_state(|| None::<Feedback>);
    let busy = use_state(|| false);

    let close = {
        let open = open.clone();
        Callback::from(move |()| open.set(Open::None))
    };
    let onsaved = {
        let (open, feedback, reload) = (open.clone(), feedback.clone(), reload.clone());
        Callback::from(move |message: String| {
            open.set(Open::None);
            feedback.set(Some(Feedback::Success(message)));
            reload.emit(());
        })
    };
    let ondelete = {
        let (open, feedback, reload, busy, config) = (
            open.clone(),
            feedback.clone(),
            reload.clone(),
            busy.clone(),
            config.clone(),
        );
        Callback::from(move |id: String| {
            let (open, feedback, reload, busy, config) = (
                open.clone(),
                feedback.clone(),
                reload.clone(),
                busy.clone(),
                config.clone(),
            );
            busy.set(true);
            spawn_local(async move {
                let request = Api::new(&config).delete_credential_definition(&id);
                let result = client::send_empty(&config, &request).await;
                busy.set(false);
                open.set(Open::None);
                feedback.set(Some(match result {
                    Ok(()) => Feedback::Success(format!("Deleted credential definition {id}.")),
                    Err(e) => Feedback::error("Could not delete credential definition", e),
                }));
                reload.emit(());
            });
        })
    };

    let open_create = {
        let open = open.clone();
        Callback::from(move |_| open.set(Open::Create))
    };

    let body = match &definitions {
        Load::Loading => html!(<Loading label="Loading credential definitions" />),
        Load::Failed(message) => html!(
            <ErrorAlert title="Could not load credential definitions" message={message.clone()} />
        ),
        Load::Ready(list) => definitions_table(list, &open),
    };

    let dialog = match &*open {
        Open::None => html!(),
        Open::Create => {
            html!(<DefinitionDialog existing={None} onclose={close.clone()} onsaved={onsaved.clone()} />)
        }
        Open::Edit(def) => {
            html!(<DefinitionDialog existing={Some(def.clone())} onclose={close.clone()} onsaved={onsaved.clone()} />)
        }
        Open::Delete(id) => {
            let id_for_confirm = id.clone();
            html! {
                <ConfirmDialog
                    title="Delete credential definition"
                    confirm_label="Delete"
                    busy={*busy}
                    onclose={close.clone()}
                    onconfirm={ondelete.reform(move |()| id_for_confirm.clone())}
                >
                    <p>
                        { "Delete the credential definition " }<strong>{ id.clone() }</strong>{ "? " }
                        { "Holders can no longer request it. The hub refuses while an issuance process still references it." }
                    </p>
                </ConfirmDialog>
            }
        }
    };

    html! {
        <>
            <PageHeader
                title="Credential Definitions"
                description="The credential types this issuer offers to holders (listed in the public DCP metadata)."
            />
            <PageSection>
                <FeedbackAlert feedback={(*feedback).clone()} onclose={{ let f = feedback.clone(); Callback::from(move |()| f.set(None)) }} />
                <div class="pf-v6-u-mb-md">
                    <Button variant={ButtonVariant::Primary} label="Create credential definition" onclick={open_create} />
                </div>
                { body }
            </PageSection>
            { dialog }
        </>
    }
}

fn definitions_table(list: &[CredentialDefinition], open: &UseStateHandle<Open>) -> Html {
    let rows = if list.is_empty() {
        empty_row(
            COLUMNS.len(),
            "No credential definitions",
            html!("Create one so holders can request credentials of that type."),
        )
    } else {
        list.iter()
            .map(|def| {
                let edit = {
                    let (open, def) = (open.clone(), def.clone());
                    Callback::from(move |_| open.set(Open::Edit(def.clone())))
                };
                let delete = {
                    let (open, id) = (open.clone(), def.id.clone());
                    Callback::from(move |_| open.set(Open::Delete(id.clone())))
                };
                html! {
                    <tr key={def.id.clone()}>
                        { long_cell("ID", &def.id) }
                        { cell("Credential type", def.credential_type.clone()) }
                        { cell("Format", def.format.clone()) }
                        <td data-label="Validity (seconds)" title={duration(def.validity)}>
                            { def.validity }
                            <span class="pf-v6-u-color-200">{ format!(" ({})", duration(def.validity)) }</span>
                        </td>
                        { cell("Created", timestamp_or(def.created_at, "-")) }
                        <td class="pf-v6-c-table__action" data-label="Actions">
                            <div class="pf-v6-l-flex pf-m-nowrap pf-m-gap-sm">
                                <Button variant={ButtonVariant::Secondary} label="Edit" onclick={edit} />
                                <Button variant={ButtonVariant::DangerSecondary} label="Delete" onclick={delete} />
                            </div>
                        </td>
                    </tr>
                }
            })
            .collect::<Html>()
    };
    html! {
        <table class="pf-v6-c-table pf-m-grid-md" aria-label={TABLE}>
            { header_row(&COLUMNS) }
            <tbody>{ rows }</tbody>
        </table>
    }
}

#[derive(Clone, PartialEq, Properties)]
struct DefinitionDialogProps {
    existing: Option<CredentialDefinition>,
    onclose: Callback<()>,
    onsaved: Callback<String>,
}

/// Create (no `existing`) or edit form. Stays open on failure, with the
/// backend's message in an alert.
#[function_component(DefinitionDialog)]
fn definition_dialog(props: &DefinitionDialogProps) -> Html {
    let config = use_config();
    let editing = props.existing.is_some();
    let id = use_state(|| {
        props
            .existing
            .as_ref()
            .map(|d| d.id.clone())
            .unwrap_or_default()
    });
    let credential_type = use_state(|| {
        props
            .existing
            .as_ref()
            .map(|d| d.credential_type.clone())
            .unwrap_or_default()
    });
    let validity = use_state(|| {
        props.existing.as_ref().map_or_else(
            || crate::api::DEFAULT_VALIDITY.to_string(),
            |d| d.validity.to_string(),
        )
    });
    let errors = use_state(DefinitionErrors::default);
    let failure = use_state(|| None::<String>);
    let busy = use_state(|| false);

    let submit = {
        let (id, credential_type, validity, errors, failure, busy) = (
            id.clone(),
            credential_type.clone(),
            validity.clone(),
            errors.clone(),
            failure.clone(),
            busy.clone(),
        );
        let (existing, onsaved, config) = (
            props.existing.clone(),
            props.onsaved.clone(),
            config.clone(),
        );
        Callback::from(move |()| {
            let definition = match validate_definition(&id, &credential_type, &validity) {
                Ok(def) => def,
                Err(e) => {
                    errors.set(e);
                    return;
                }
            };
            errors.set(DefinitionErrors::default());
            // Keep what the form does not show (schema, mappings, ...).
            let definition = match &existing {
                Some(old) => CredentialDefinition {
                    credential_type: definition.credential_type,
                    validity: definition.validity,
                    ..old.clone()
                },
                None => definition,
            };
            let (failure, busy, onsaved, config, editing) = (
                failure.clone(),
                busy.clone(),
                onsaved.clone(),
                config.clone(),
                existing.is_some(),
            );
            busy.set(true);
            spawn_local(async move {
                let api = Api::new(&config);
                let request = if editing {
                    api.update_credential_definition(&definition)
                } else {
                    api.create_credential_definition(&definition)
                };
                let result = client::send_empty(&config, &request).await;
                busy.set(false);
                match result {
                    Ok(()) => onsaved.emit(format!(
                        "{} credential definition {}.",
                        if editing { "Saved" } else { "Created" },
                        definition.id
                    )),
                    Err(e) => failure.set(Some(e.to_string())),
                }
            });
        })
    };

    let set = |state: &UseStateHandle<String>| {
        let state = state.clone();
        Callback::from(move |v: String| state.set(v))
    };
    let (title, submit_label) = if editing {
        ("Edit credential definition", "Save")
    } else {
        ("Create credential definition", "Create")
    };

    html! {
        <Dialog
            title={title}
            onclose={props.onclose.clone()}
            footer={form_footer("definition-form", submit_label, *busy, &props.onclose)}
        >
            if let Some(message) = &*failure {
                <ErrorAlert
                    title={if editing { "Could not save credential definition" } else { "Could not create credential definition" }}
                    message={message.clone()}
                />
            }
            <form id="definition-form" class="pf-v6-c-form" novalidate=true onsubmit={on_submit(submit)}>
                <TextField
                    id="definition-id"
                    label="ID"
                    value={(*id).clone()}
                    oninput={set(&id)}
                    readonly={editing}
                    error={errors.id.clone()}
                    helper={(!editing).then_some(AttrValue::Static("Unique key, e.g. membership-credential. Cannot be changed later."))}
                />
                <TextField
                    id="definition-type"
                    label="Credential type"
                    value={(*credential_type).clone()}
                    oninput={set(&credential_type)}
                    error={errors.credential_type.clone()}
                    helper={AttrValue::Static("The VC type, e.g. MembershipCredential.")}
                />
                <TextField
                    id="definition-validity"
                    label="Validity (seconds)"
                    input_type="number"
                    inputmode="numeric"
                    value={(*validity).clone()}
                    oninput={set(&validity)}
                    error={errors.validity.clone()}
                    helper={validity.parse::<u64>().ok().filter(|v| *v > 0).map(|v| AttrValue::from(format!("Issued credentials expire after {}.", duration(v))))}
                />
                <TextField
                    id="definition-format"
                    label="Format"
                    value={crate::api::FORMAT_VC1_0_JWT}
                    oninput={Callback::noop()}
                    readonly=true
                    helper={AttrValue::Static("The only format the hub issues today.")}
                />
            </form>
        </Dialog>
    }
}
