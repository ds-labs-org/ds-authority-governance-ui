//! `/trusted-issuers`: the trusted issuers registry (Root TAO -> TAO ->
//! Trusted issuer accreditation chain), with a trust check. Read-only
//! when the registry admin API is not available to this console.

use patternfly_yew::prelude::*;
use yew::platform::spawn_local;
use yew::prelude::*;

use crate::api::{
    Api, ApiError, IssuerFilter, IssuerList, IssuerRecord, RegistryStatus, Role, TrustDecision,
    client,
};
use crate::components::*;
use crate::format::{date_input, timestamp_or};
use crate::hooks::{Load, use_config, use_load};
use crate::validation::{IssuerErrors, IssuerForm};

const TABLE: &str = "Trusted issuers";
const COLUMNS: [&str; 8] = [
    "DID",
    "Name",
    "Role",
    "Credential types",
    "Accredited by",
    "Status",
    "Valid until",
    "",
];

#[derive(Clone, PartialEq)]
enum Open {
    None,
    Add,
    Edit(IssuerRecord),
    Revoke(IssuerRecord),
    Delete(IssuerRecord),
}

pub fn registry_status_label(status: RegistryStatus) -> Html {
    let color = match status {
        RegistryStatus::Active => Color::Green,
        RegistryStatus::Suspended => Color::Orange,
        RegistryStatus::Revoked => Color::Red,
    };
    status_label(status.label(), color)
}

/// `None` when the console may write; `Some(reason)` when read-only.
async fn probe_admin(config: crate::config::Config) -> Result<Option<String>, ApiError> {
    let request = Api::new(&config).registry_admin_probe();
    Ok(match client::send_raw(&config, &request).await {
        Ok((status, _)) if (200..300).contains(&status) => None,
        Ok((status, body)) => Some(ApiError::from_response(status, &body).to_string()),
        Err(e) => Some(e.to_string()),
    })
}

async fn list(config: crate::config::Config, filter: IssuerFilter) -> Result<IssuerList, ApiError> {
    let request = Api::new(&config).list_issuers(&filter);
    client::send_json::<IssuerList>(&config, &request).await
}

#[function_component(TrustedIssuers)]
pub fn trusted_issuers() -> Html {
    let config = use_config();
    let role_filter = use_state(String::new);
    let status_filter = use_state(String::new);

    let (admin, _) = use_load(probe_admin);
    let filter = IssuerFilter {
        role: Role::parse(&role_filter),
        status: RegistryStatus::parse(&status_filter),
        credential_type: None,
    };
    let (issuers, reload_issuers) = use_load(move |config| list(config, filter));
    // Every active record that may accredit, for the "Accredited by"
    // suggestions (independent of the table filters).
    let (active, reload_active) = use_load(|config| async move {
        let filter = IssuerFilter {
            status: Some(RegistryStatus::Active),
            ..Default::default()
        };
        list(config, filter).await.map(|l| l.items)
    });

    let open = use_state(|| Open::None);
    let feedback = use_state(|| None::<Feedback>);
    let busy = use_state(|| false);

    let writable = matches!(admin, Load::Ready(None));
    let reload = {
        let (a, b) = (reload_issuers.clone(), reload_active.clone());
        Callback::from(move |()| {
            a.emit(());
            b.emit(());
        })
    };
    let close = {
        let open = open.clone();
        Callback::from(move |()| open.set(Open::None))
    };

    let set_status = {
        let (open, feedback, reload, busy, config) = (
            open.clone(),
            feedback.clone(),
            reload.clone(),
            busy.clone(),
            config.clone(),
        );
        Callback::from(move |(did, status): (String, RegistryStatus)| {
            let (open, feedback, reload, busy, config) = (
                open.clone(),
                feedback.clone(),
                reload.clone(),
                busy.clone(),
                config.clone(),
            );
            busy.set(true);
            spawn_local(async move {
                let request = Api::new(&config).set_issuer_status(&did, status);
                let result = client::send_empty(&config, &request).await;
                busy.set(false);
                open.set(Open::None);
                let verb = match status {
                    RegistryStatus::Active => "activate",
                    RegistryStatus::Suspended => "suspend",
                    RegistryStatus::Revoked => "revoke",
                };
                feedback.set(Some(match result {
                    Ok(()) => Feedback::Success(format!("{did} is now {}.", status.as_str())),
                    Err(e) => Feedback::error(format!("Could not {verb} trusted issuer"), e),
                }));
                reload.emit(());
            });
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
        Callback::from(move |did: String| {
            let (open, feedback, reload, busy, config) = (
                open.clone(),
                feedback.clone(),
                reload.clone(),
                busy.clone(),
                config.clone(),
            );
            busy.set(true);
            spawn_local(async move {
                let request = Api::new(&config).delete_issuer(&did);
                let result = client::send_empty(&config, &request).await;
                busy.set(false);
                open.set(Open::None);
                feedback.set(Some(match result {
                    Ok(()) => Feedback::Success(format!("Deleted {did} from the registry.")),
                    Err(e) => Feedback::error("Could not delete trusted issuer", e),
                }));
                reload.emit(());
            });
        })
    };

    let onsaved = {
        let (open, feedback, reload) = (open.clone(), feedback.clone(), reload.clone());
        Callback::from(move |message: String| {
            open.set(Open::None);
            feedback.set(Some(Feedback::Success(message)));
            reload.emit(());
        })
    };

    let onfilter = |state: &UseStateHandle<String>| {
        let (state, reload) = (state.clone(), reload_issuers.clone());
        Callback::from(move |e: Event| {
            state.set(
                e.target_unchecked_into::<web_sys::HtmlSelectElement>()
                    .value(),
            );
            reload.emit(());
        })
    };
    let role_options: Vec<(String, String)> =
        std::iter::once((String::new(), "All roles".to_string()))
            .chain(
                Role::ALL
                    .iter()
                    .map(|r| (r.as_str().to_string(), r.label().to_string())),
            )
            .collect();
    let status_options: Vec<(String, String)> =
        std::iter::once((String::new(), "All statuses".to_string()))
            .chain(
                RegistryStatus::ALL
                    .iter()
                    .map(|s| (s.as_str().to_string(), s.label().to_string())),
            )
            .collect();

    let accreditors: Vec<IssuerRecord> = active
        .ready()
        .map(|items| {
            items
                .iter()
                .filter(|r| r.role.can_accredit())
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    let known: Vec<IssuerRecord> = issuers.ready().map(|l| l.items.clone()).unwrap_or_default();

    let table = match &issuers {
        Load::Loading => html!(<Loading label="Loading trusted issuers" />),
        Load::Failed(message) => {
            html!(<ErrorAlert title="Could not load trusted issuers" message={message.clone()} />)
        }
        Load::Ready(list) => {
            let filtered = !role_filter.is_empty() || !status_filter.is_empty();
            issuers_table(&list.items, filtered, writable, *busy, &open, &set_status)
        }
    };

    let dialog = match &*open {
        Open::None => html!(),
        Open::Add => {
            html!(<IssuerDialog existing={None} accreditors={accreditors.clone()} onclose={close.clone()} onsaved={onsaved.clone()} />)
        }
        Open::Edit(record) => {
            html!(<IssuerDialog existing={Some(record.clone())} accreditors={accreditors.clone()} onclose={close.clone()} onsaved={onsaved.clone()} />)
        }
        Open::Revoke(record) => {
            let did = record.did.clone();
            html! {
                <ConfirmDialog
                    title="Revoke trusted issuer"
                    confirm_label="Revoke"
                    busy={*busy}
                    onclose={close.clone()}
                    onconfirm={set_status.reform(move |()| (did.clone(), RegistryStatus::Revoked))}
                >
                    <p>{ "Revoke " }<strong class="pf-v6-u-text-break-word">{ record.did.clone() }</strong>{ "?" }</p>
                    <p class="pf-v6-u-mt-sm">
                        { "Its credentials, and those of every party it accredited, stop being trusted. Use Suspend for a temporary stop." }
                    </p>
                </ConfirmDialog>
            }
        }
        Open::Delete(record) => {
            let did = record.did.clone();
            html! {
                <ConfirmDialog
                    title="Delete trusted issuer"
                    confirm_label="Delete"
                    busy={*busy}
                    onclose={close.clone()}
                    onconfirm={ondelete.reform(move |()| did.clone())}
                >
                    <p>{ "Delete " }<strong class="pf-v6-u-text-break-word">{ record.did.clone() }</strong>{ " from the registry?" }</p>
                    <p class="pf-v6-u-mt-sm">
                        { "The registry refuses while it still accredits other parties." }
                    </p>
                </ConfirmDialog>
            }
        }
    };

    let add = {
        let open = open.clone();
        Callback::from(move |_| open.set(Open::Add))
    };

    html! {
        <>
            <PageHeader
                title="Trusted Issuers"
                description="Who may issue which credential types in this dataspace, and who accredited them."
            />
            <PageSection>
                if let Load::Ready(Some(reason)) = &admin {
                    <InlineAlert kind={Notice::Info} title="The registry is read-only for this console">
                        <p>{ "Records can be browsed and trust checked, but not changed: the registry admin API is not available (" }{ reason.clone() }{ ")." }</p>
                    </InlineAlert>
                }
                <FeedbackAlert feedback={(*feedback).clone()} onclose={{ let f = feedback.clone(); Callback::from(move |()| f.set(None)) }} />
                <div class="pf-v6-l-flex pf-m-align-items-flex-end pf-m-gap-md pf-v6-u-mb-md">
                    <Button variant={ButtonVariant::Primary} label="Add trusted issuer" onclick={add} disabled={!writable} />
                    <div>
                        <label class="pf-v6-c-form__label" for="issuer-role-filter">
                            <span class="pf-v6-c-form__label-text">{ "Filter by role" }</span>
                        </label>
                        { select_control("issuer-role-filter".into(), &role_filter, &role_options, onfilter(&role_filter), false, false) }
                    </div>
                    <div>
                        <label class="pf-v6-c-form__label" for="issuer-status-filter">
                            <span class="pf-v6-c-form__label-text">{ "Filter by status" }</span>
                        </label>
                        { select_control("issuer-status-filter".into(), &status_filter, &status_options, onfilter(&status_filter), false, false) }
                    </div>
                </div>
                { table }
            </PageSection>
            <PageSection>
                <TrustCheck known={known} />
            </PageSection>
            { dialog }
        </>
    }
}

fn issuers_table(
    items: &[IssuerRecord],
    filtered: bool,
    writable: bool,
    busy: bool,
    open: &UseStateHandle<Open>,
    set_status: &Callback<(String, RegistryStatus)>,
) -> Html {
    let rows = if items.is_empty() {
        if filtered {
            empty_row(
                COLUMNS.len(),
                "No trusted issuers match these filters",
                html!(),
            )
        } else {
            empty_row(
                COLUMNS.len(),
                "No trusted issuers",
                html!("Start with the dataspace authority as Root TAO."),
            )
        }
    } else {
        items
            .iter()
            .map(|record| {
                let opener = |make: fn(IssuerRecord) -> Open| {
                    let (open, record) = (open.clone(), record.clone());
                    Callback::from(move |_: MouseEvent| open.set(make(record.clone())))
                };
                let status_to = |status: RegistryStatus| {
                    let did = record.did.clone();
                    set_status.reform(move |_: MouseEvent| (did.clone(), status))
                };
                let disabled = !writable || busy;
                html! {
                    <tr class="pf-v6-c-table__tr" key={record.did.clone()}>
                        { long_cell("DID", &record.did) }
                        { cell("Name", record.name.clone().unwrap_or_else(|| "-".into())) }
                        { cell("Role", record.role.label()) }
                        { cell("Credential types", record.credential_types.join(", ")) }
                        { match &record.accredited_by {
                            Some(did) => long_cell("Accredited by", did),
                            None => cell("Accredited by", "-"),
                        } }
                        { cell("Status", registry_status_label(record.status)) }
                        { cell("Valid until", record.valid_until.map_or_else(|| "No expiry".to_string(), |t| timestamp_or(Some(t), ""))) }
                        <td class="pf-v6-c-table__td pf-v6-c-table__action" data-label="Actions">
                            <div class="pf-v6-l-flex pf-m-gap-xs" style="min-width: 13rem;">
                                <Button size={ButtonSize::Small} variant={ButtonVariant::Secondary} label="Edit" onclick={opener(Open::Edit)} {disabled} />
                                if record.status == RegistryStatus::Active {
                                    <Button size={ButtonSize::Small} variant={ButtonVariant::Secondary} label="Suspend" onclick={status_to(RegistryStatus::Suspended)} {disabled} />
                                } else {
                                    <Button size={ButtonSize::Small} variant={ButtonVariant::Secondary} label="Activate" onclick={status_to(RegistryStatus::Active)} {disabled} />
                                }
                                if record.status != RegistryStatus::Revoked {
                                    <Button size={ButtonSize::Small} variant={ButtonVariant::DangerSecondary} label="Revoke" onclick={opener(Open::Revoke)} {disabled} />
                                }
                                <Button size={ButtonSize::Small} variant={ButtonVariant::DangerSecondary} label="Delete" onclick={opener(Open::Delete)} {disabled} />
                            </div>
                        </td>
                    </tr>
                }
            })
            .collect::<Html>()
    };
    html! {
        <div class="pf-v6-u-overflow-x-auto">
            <table class="pf-v6-c-table pf-m-grid-lg pf-m-compact" aria-label={TABLE}>
                { header_row(&COLUMNS) }
                <tbody class="pf-v6-c-table__tbody">{ rows }</tbody>
            </table>
        </div>
    }
}

#[derive(Clone, PartialEq, Properties)]
struct IssuerDialogProps {
    existing: Option<IssuerRecord>,
    accreditors: Vec<IssuerRecord>,
    onclose: Callback<()>,
    onsaved: Callback<String>,
}

#[function_component(IssuerDialog)]
fn issuer_dialog(props: &IssuerDialogProps) -> Html {
    let config = use_config();
    let form = use_state(|| match &props.existing {
        None => IssuerForm::default(),
        Some(r) => IssuerForm {
            did: r.did.clone(),
            name: r.name.clone().unwrap_or_default(),
            role: r.role.as_str().to_string(),
            credential_types: r.credential_types.join(", "),
            accredited_by: r.accredited_by.clone().unwrap_or_default(),
            valid_from: r.valid_from.map(date_input).unwrap_or_default(),
            valid_until: r.valid_until.map(date_input).unwrap_or_default(),
        },
    });
    let errors = use_state(IssuerErrors::default);
    let failure = use_state(|| None::<String>);
    let busy = use_state(|| false);
    let editing = props.existing.is_some();

    let submit = {
        let (form, errors, failure, busy) =
            (form.clone(), errors.clone(), failure.clone(), busy.clone());
        let (existing, onsaved, config) = (
            props.existing.clone(),
            props.onsaved.clone(),
            config.clone(),
        );
        Callback::from(move |()| {
            let status = existing
                .as_ref()
                .map_or(RegistryStatus::Active, |r| r.status);
            let record = match form.validate(status) {
                Ok(r) => r,
                Err(e) => {
                    errors.set(e);
                    return;
                }
            };
            errors.set(IssuerErrors::default());
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
                // PUT is an upsert: never overwrite an existing record from "Add".
                if !editing {
                    match client::send_raw(&config, &api.get_issuer(&record.did)).await {
                        Ok((404, _)) => {}
                        Ok((status, _)) if (200..300).contains(&status) => {
                            busy.set(false);
                            failure.set(Some(format!(
                                "Conflict: {} is already in the registry. Edit it instead.",
                                record.did
                            )));
                            return;
                        }
                        Ok((status, body)) => {
                            busy.set(false);
                            failure.set(Some(ApiError::from_response(status, &body).to_string()));
                            return;
                        }
                        Err(e) => {
                            busy.set(false);
                            failure.set(Some(e.to_string()));
                            return;
                        }
                    }
                }
                let result = client::send_empty(&config, &api.put_issuer(&record)).await;
                busy.set(false);
                match result {
                    Ok(()) => onsaved.emit(format!(
                        "{} {}.",
                        if editing { "Saved" } else { "Added" },
                        record.did
                    )),
                    Err(e) => failure.set(Some(e.to_string())),
                }
            });
        })
    };

    let field = |apply: fn(&mut IssuerForm, String)| {
        let form = form.clone();
        Callback::from(move |value: String| {
            let mut next = (*form).clone();
            apply(&mut next, value);
            form.set(next);
        })
    };
    let role_options: Vec<(String, String)> =
        std::iter::once((String::new(), "Select a role".to_string()))
            .chain(
                Role::ALL
                    .iter()
                    .map(|r| (r.as_str().to_string(), r.label().to_string())),
            )
            .collect();
    let title = if editing {
        "Edit trusted issuer"
    } else {
        "Add trusted issuer"
    };
    let accreditor_options: Vec<&IssuerRecord> = props
        .accreditors
        .iter()
        .filter(|r| r.did != form.did.trim())
        .collect();

    html! {
        <Dialog
            title={title}
            onclose={props.onclose.clone()}
            footer={form_footer("issuer-form", "Save", *busy, &props.onclose)}
        >
            if let Some(message) = &*failure {
                <ErrorAlert title="Could not save trusted issuer" message={message.clone()} />
            }
            <form id="issuer-form" class="pf-v6-c-form" novalidate=true onsubmit={on_submit(submit)}>
                <TextField
                    id="issuer-did"
                    label="DID"
                    value={form.did.clone()}
                    oninput={field(|f, v| f.did = v)}
                    readonly={editing}
                    error={errors.did.clone()}
                    placeholder="did:web:issuer.example"
                />
                <TextField id="issuer-name" label="Name" value={form.name.clone()} oninput={field(|f, v| f.name = v)} />
                <SelectField
                    id="issuer-role"
                    label="Role"
                    value={form.role.clone()}
                    options={role_options}
                    onchange={field(|f, v| f.role = v)}
                    error={errors.role.clone()}
                />
                <TextField
                    id="issuer-types"
                    label="Credential types"
                    value={form.credential_types.clone()}
                    oninput={field(|f, v| f.credential_types = v)}
                    error={errors.credential_types.clone()}
                    helper={AttrValue::Static("Comma-separated, e.g. MembershipCredential. A Root TAO may use * for any type.")}
                />
                <TextField
                    id="issuer-accredited-by"
                    label="Accredited by"
                    value={form.accredited_by.clone()}
                    oninput={field(|f, v| f.accredited_by = v)}
                    error={errors.accredited_by.clone()}
                    list={AttrValue::Static("issuer-accreditors")}
                    helper={AttrValue::Static("DID of an active Root TAO or TAO (suggestions offered). Leave empty for a Root TAO.")}
                />
                <datalist id="issuer-accreditors">
                    { for accreditor_options.iter().map(|r| html! {
                        <option value={r.did.clone()}>
                            { format!("{} ({})", r.name.clone().unwrap_or_else(|| r.did.clone()), r.role.label()) }
                        </option>
                    }) }
                </datalist>
                <div class="pf-v6-l-grid pf-m-gutter pf-m-all-6-col-on-md">
                    <TextField
                        id="issuer-valid-from"
                        label="Valid from"
                        input_type="date"
                        value={form.valid_from.clone()}
                        oninput={field(|f, v| f.valid_from = v)}
                        error={errors.valid_from.clone()}
                        helper={AttrValue::Static("Optional (UTC).")}
                    />
                    <TextField
                        id="issuer-valid-until"
                        label="Valid until"
                        input_type="date"
                        value={form.valid_until.clone()}
                        oninput={field(|f, v| f.valid_until = v)}
                        error={errors.valid_until.clone()}
                        helper={AttrValue::Static("Optional, exclusive (UTC).")}
                    />
                </div>
            </form>
        </Dialog>
    }
}

#[derive(Clone, PartialEq)]
enum Check {
    Idle,
    Checking,
    Done {
        did: String,
        credential_type: String,
        decision: TrustDecision,
    },
    Failed(String),
}

#[derive(Clone, PartialEq, Properties)]
struct TrustCheckProps {
    /// Records to annotate chain entries with.
    known: Vec<IssuerRecord>,
}

/// `GET /v1/issuers/{did}/types/{type}` as a form.
#[function_component(TrustCheck)]
fn trust_check(props: &TrustCheckProps) -> Html {
    let config = use_config();
    let did = use_state(String::new);
    let credential_type = use_state(String::new);
    let errors = use_state(|| (None::<String>, None::<String>));
    let state = use_state(|| Check::Idle);

    let submit = {
        let (did, credential_type, errors, state) = (
            did.clone(),
            credential_type.clone(),
            errors.clone(),
            state.clone(),
        );
        Callback::from(move |()| {
            let (d, t) = (did.trim().to_string(), credential_type.trim().to_string());
            let e = (
                d.is_empty().then(|| "Issuer DID is required.".to_string()),
                t.is_empty()
                    .then(|| "Credential type is required.".to_string()),
            );
            let invalid = e.0.is_some() || e.1.is_some();
            errors.set(e);
            if invalid {
                return;
            }
            let (state, config) = (state.clone(), config.clone());
            state.set(Check::Checking);
            spawn_local(async move {
                let request = Api::new(&config).check_trust(&d, &t);
                state.set(
                    match client::send_json::<TrustDecision>(&config, &request).await {
                        Ok(decision) => Check::Done {
                            did: d,
                            credential_type: t,
                            decision,
                        },
                        Err(e) => Check::Failed(e.to_string()),
                    },
                );
            });
        })
    };
    let set = |s: &UseStateHandle<String>| {
        let s = s.clone();
        Callback::from(move |v: String| s.set(v))
    };
    let describe = |chain_did: &str| -> Html {
        match props.known.iter().find(|r| r.did == chain_did) {
            Some(r) => html! {
                <>
                    <span class="pf-v6-u-text-break-word">{ chain_did.to_string() }</span>
                    <span class="pf-v6-u-color-200">
                        { format!(" - {}{}", r.role.label(), r.name.as_ref().map(|n| format!(", {n}")).unwrap_or_default()) }
                    </span>
                </>
            },
            None => html!(<span class="pf-v6-u-text-break-word">{ chain_did.to_string() }</span>),
        }
    };

    let result = match &*state {
        Check::Idle => html!(),
        Check::Checking => html!(<Loading label="Checking trust" />),
        Check::Failed(message) => {
            html!(<ErrorAlert title="Could not check trust" message={message.clone()} />)
        }
        Check::Done {
            did,
            credential_type,
            decision,
        } => html! {
            <div class="pf-v6-u-mt-md">
                <p>
                    if decision.trusted {
                        <Label label="Trusted" color={Color::Green} icon={Icon::CheckCircle} />
                    } else {
                        <Label label="Not trusted" color={Color::Red} icon={Icon::ExclamationCircle} />
                    }
                    <span class="pf-v6-u-ml-sm">
                        { format!("{credential_type} from ") }<span class="pf-v6-u-text-break-word">{ did.clone() }</span>
                    </span>
                </p>
                if let Some(reason) = &decision.reason {
                    <p class="pf-v6-u-mt-sm">{ "Reason: " }{ reason.clone() }</p>
                }
                <p class="pf-v6-u-mt-sm pf-v6-u-font-weight-bold">{ "Accreditation chain (issuer first)" }</p>
                <ol class="pf-v6-c-list" aria-label="Trust chain">
                    { for decision.chain.iter().map(|d| html! { <li>{ describe(d) }</li> }) }
                </ol>
            </div>
        },
    };

    html! {
        <section aria-labelledby="check-trust-title">
            <Title level={Level::H2} size={Size::Large}><span id="check-trust-title">{ "Check trust" }</span></Title>
            <p class="pf-v6-u-color-200 pf-v6-u-mb-sm">
                { "Ask the registry whether a DID may issue a credential type, walking its accreditation chain up to a Root TAO." }
            </p>
            <form class="pf-v6-c-form" aria-label="Check trust" novalidate=true onsubmit={on_submit(submit)}>
                <div class="pf-v6-l-grid pf-m-gutter pf-m-all-6-col-on-md">
                    <TextField
                        id="check-did"
                        label="Issuer DID"
                        value={(*did).clone()}
                        oninput={set(&did)}
                        error={errors.0.clone()}
                        list={AttrValue::Static("check-known-dids")}
                    />
                    <TextField
                        id="check-type"
                        label="Credential type"
                        value={(*credential_type).clone()}
                        oninput={set(&credential_type)}
                        error={errors.1.clone()}
                        placeholder="MembershipCredential"
                    />
                </div>
                <datalist id="check-known-dids">
                    { for props.known.iter().map(|r| html! { <option value={r.did.clone()} /> }) }
                </datalist>
                <div class="pf-v6-c-form__group pf-m-action">
                    <div class="pf-v6-c-form__actions">
                        <Button variant={ButtonVariant::Secondary} r#type={ButtonType::Submit} label="Check" disabled={*state == Check::Checking} />
                    </div>
                </div>
                <div role="status" aria-live="polite">{ result }</div>
            </form>
        </section>
    }
}
