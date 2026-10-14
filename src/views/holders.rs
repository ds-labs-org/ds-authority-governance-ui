//! `/holders`: the approval queue first, then every holder the hub knows.

use patternfly_yew::prelude::*;
use yew::platform::spawn_local;
use yew::prelude::*;

use crate::api::{Api, Holder, HolderStatus, QuerySpec, client};
use crate::components::*;
use crate::format::timestamp;
use crate::hooks::{Load, use_config, use_load};
use crate::validation::{HolderErrors, validate_holder};

const PENDING_COLUMNS: [&str; 4] = ["DID", "Name", "Requested", ""];
const ALL_COLUMNS: [&str; 5] = ["DID", "Name", "Status", "Holder ID", "Created"];

#[derive(Clone, PartialEq)]
enum Open {
    None,
    Register,
    Reject(Holder),
}

pub fn holder_status_label(status: HolderStatus) -> Html {
    let color = match status {
        HolderStatus::Pending => Color::Orange,
        HolderStatus::Approved => Color::Green,
        HolderStatus::Rejected => Color::Red,
        HolderStatus::Unknown => Color::Grey,
    };
    status_label(status.label(), color)
}

#[function_component(Holders)]
pub fn holders() -> Html {
    let config = use_config();
    let (holders, reload) = use_load(|config| async move {
        let request = Api::new(&config).query_holders(&QuerySpec::all_newest_first("createdAt"));
        client::send_json::<Vec<Holder>>(&config, &request).await
    });
    let open = use_state(|| Open::None);
    let feedback = use_state(|| None::<Feedback>);
    let busy = use_state(|| None::<String>);
    let filter = use_state(String::new);

    let close = {
        let open = open.clone();
        Callback::from(move |()| open.set(Open::None))
    };

    // Approve (immediate) and reject (after confirmation) share this.
    let decide = {
        let (open, feedback, reload, busy, config) = (
            open.clone(),
            feedback.clone(),
            reload.clone(),
            busy.clone(),
            config.clone(),
        );
        Callback::from(move |(holder, approve): (Holder, bool)| {
            let (open, feedback, reload, busy, config) = (
                open.clone(),
                feedback.clone(),
                reload.clone(),
                busy.clone(),
                config.clone(),
            );
            busy.set(Some(holder.holder_id.clone()));
            spawn_local(async move {
                let api = Api::new(&config);
                let request = if approve {
                    api.approve_holder(&holder.holder_id)
                } else {
                    api.reject_holder(&holder.holder_id)
                };
                let result = client::send_empty(&config, &request).await;
                busy.set(None);
                open.set(Open::None);
                feedback.set(Some(match (result, approve) {
                    (Ok(()), true) => Feedback::Success(format!(
                        "Approved {}. Its pending credential requests are being issued.",
                        holder.did
                    )),
                    (Ok(()), false) => Feedback::Success(format!(
                        "Rejected {}. Its pending credential requests were rejected.",
                        holder.did
                    )),
                    (Err(e), true) => Feedback::error("Could not approve holder", e),
                    (Err(e), false) => Feedback::error("Could not reject holder", e),
                }));
                reload.emit(());
            });
        })
    };

    let onregistered = {
        let (open, feedback, reload) = (open.clone(), feedback.clone(), reload.clone());
        Callback::from(move |did: String| {
            open.set(Open::None);
            feedback.set(Some(Feedback::Success(format!("Registered holder {did}."))));
            reload.emit(());
        })
    };

    let content = match &holders {
        Load::Loading => html!(<Loading label="Loading holders" />),
        Load::Failed(message) => {
            html!(<ErrorAlert title="Could not load holders" message={message.clone()} />)
        }
        Load::Ready(list) => {
            let pending: Vec<&Holder> = list
                .iter()
                .filter(|h| h.status == HolderStatus::Pending)
                .collect();
            let shown: Vec<&Holder> = list
                .iter()
                .filter(|h| filter.is_empty() || h.status.as_str() == filter.as_str())
                .collect();
            let onfilter = {
                let filter = filter.clone();
                Callback::from(move |e: Event| {
                    filter.set(
                        e.target_unchecked_into::<web_sys::HtmlSelectElement>()
                            .value(),
                    )
                })
            };
            let options: Vec<(String, String)> =
                std::iter::once((String::new(), "All statuses".to_string()))
                    .chain(
                        HolderStatus::ALL
                            .iter()
                            .map(|s| (s.as_str().to_string(), s.label().to_string())),
                    )
                    .collect();
            html! {
                <>
                    <section class="pf-v6-u-mb-xl" aria-labelledby="pending-title">
                        <Title level={Level::H2} size={Size::Large}>
                            <span id="pending-title">{ "Pending approval" }</span>
                            { " " }<Badge read=false>{ pending.len().to_string() }</Badge>
                        </Title>
                        <p class="pf-v6-u-color-200 pf-v6-u-mb-sm">
                            { "Holders that requested a credential and wait for a decision. Approving issues every waiting request; rejecting refuses them and any later ones." }
                        </p>
                        { pending_table(&pending, &open, &decide, busy.as_deref()) }
                    </section>
                    <section aria-labelledby="all-title">
                        <Title level={Level::H2} size={Size::Large}>
                            <span id="all-title">{ "All holders" }</span>
                        </Title>
                        <div class="pf-v6-l-flex pf-m-align-items-center pf-m-gap-sm pf-v6-u-my-sm">
                            <label class="pf-v6-c-form__label" for="holder-status-filter">
                                <span class="pf-v6-c-form__label-text">{ "Filter by status" }</span>
                            </label>
                            <div>{ select_control("holder-status-filter".into(), &filter, &options, onfilter, false, false) }</div>
                        </div>
                        { all_table(&shown, list.is_empty()) }
                    </section>
                </>
            }
        }
    };

    let dialog = match &*open {
        Open::None => html!(),
        Open::Register => {
            html!(<RegisterDialog onclose={close.clone()} onregistered={onregistered} />)
        }
        Open::Reject(holder) => {
            let target = holder.clone();
            html! {
                <ConfirmDialog
                    title="Reject holder"
                    confirm_label="Reject"
                    busy={busy.is_some()}
                    onclose={close.clone()}
                    onconfirm={decide.reform(move |()| (target.clone(), false))}
                >
                    <p>
                        { "Reject " }<strong class="pf-v6-u-text-break-word">{ holder.did.clone() }</strong>
                        { holder.name.as_ref().map(|n| format!(" ({n})")).unwrap_or_default() }{ "?" }
                    </p>
                    <p class="pf-v6-u-mt-sm">
                        { "Its waiting credential requests are rejected and the hub refuses further requests from this DID." }
                    </p>
                </ConfirmDialog>
            }
        }
    };

    let register = {
        let open = open.clone();
        Callback::from(move |_| open.set(Open::Register))
    };
    let refresh = reload.reform(|_| ());

    html! {
        <>
            <PageHeader
                title="Holders"
                description="Participants that request credentials from this issuer."
            />
            <PageSection>
                <FeedbackAlert feedback={(*feedback).clone()} onclose={{ let f = feedback.clone(); Callback::from(move |()| f.set(None)) }} />
                <div class="pf-v6-l-flex pf-m-gap-sm pf-v6-u-mb-md">
                    <Button variant={ButtonVariant::Primary} label="Register holder" onclick={register} />
                    <Button variant={ButtonVariant::Secondary} label="Refresh" icon={Icon::SyncAlt} onclick={refresh} />
                </div>
                { content }
            </PageSection>
            { dialog }
        </>
    }
}

fn pending_table(
    pending: &[&Holder],
    open: &UseStateHandle<Open>,
    decide: &Callback<(Holder, bool)>,
    busy: Option<&str>,
) -> Html {
    let rows = if pending.is_empty() {
        empty_row(
            PENDING_COLUMNS.len(),
            "No pending approvals",
            html!("Nothing waits for a decision."),
        )
    } else {
        pending
            .iter()
            .map(|holder| {
                let approve = {
                    let holder = (*holder).clone();
                    decide.reform(move |_| (holder.clone(), true))
                };
                let reject = {
                    let (open, holder) = (open.clone(), (*holder).clone());
                    Callback::from(move |_| open.set(Open::Reject(holder.clone())))
                };
                let disabled = busy.is_some_and(|id| id == holder.holder_id);
                html! {
                    <tr key={holder.holder_id.clone()}>
                        { long_cell("DID", &holder.did) }
                        { cell("Name", holder.name.clone().unwrap_or_else(|| "-".into())) }
                        { cell("Requested", timestamp(holder.created_at)) }
                        <td class="pf-v6-c-table__action" data-label="Actions">
                            <div class="pf-v6-l-flex pf-m-nowrap pf-m-gap-sm">
                                <Button variant={ButtonVariant::Primary} label="Approve" onclick={approve} {disabled} />
                                <Button variant={ButtonVariant::DangerSecondary} label="Reject" onclick={reject} {disabled} />
                            </div>
                        </td>
                    </tr>
                }
            })
            .collect::<Html>()
    };
    html! {
        <table class="pf-v6-c-table pf-m-grid-md" aria-label="Pending approval">
            { header_row(&PENDING_COLUMNS) }
            <tbody>{ rows }</tbody>
        </table>
    }
}

fn all_table(holders: &[&Holder], none_at_all: bool) -> Html {
    let rows = if holders.is_empty() {
        if none_at_all {
            empty_row(
                ALL_COLUMNS.len(),
                "No holders",
                html!("Holders appear when they request a credential or are registered here."),
            )
        } else {
            empty_row(ALL_COLUMNS.len(), "No holders match this filter", html!())
        }
    } else {
        holders
            .iter()
            .map(|holder| {
                html! {
                    <tr key={holder.holder_id.clone()}>
                        { long_cell("DID", &holder.did) }
                        { cell("Name", holder.name.clone().unwrap_or_else(|| "-".into())) }
                        { cell("Status", holder_status_label(holder.status)) }
                        { long_cell("Holder ID", &holder.holder_id) }
                        { cell("Created", timestamp(holder.created_at)) }
                    </tr>
                }
            })
            .collect::<Html>()
    };
    html! {
        <table class="pf-v6-c-table pf-m-grid-md" aria-label="All holders">
            { header_row(&ALL_COLUMNS) }
            <tbody>{ rows }</tbody>
        </table>
    }
}

#[derive(Clone, PartialEq, Properties)]
struct RegisterDialogProps {
    onclose: Callback<()>,
    onregistered: Callback<String>,
}

#[function_component(RegisterDialog)]
fn register_dialog(props: &RegisterDialogProps) -> Html {
    let config = use_config();
    let did = use_state(String::new);
    let name = use_state(String::new);
    let errors = use_state(HolderErrors::default);
    let failure = use_state(|| None::<String>);
    let busy = use_state(|| false);

    let submit = {
        let (did, name, errors, failure, busy) = (
            did.clone(),
            name.clone(),
            errors.clone(),
            failure.clone(),
            busy.clone(),
        );
        let (onregistered, config) = (props.onregistered.clone(), config.clone());
        Callback::from(move |()| {
            let holder = match validate_holder(&did, &name) {
                Ok(h) => h,
                Err(e) => {
                    errors.set(e);
                    return;
                }
            };
            errors.set(HolderErrors::default());
            let (failure, busy, onregistered, config) = (
                failure.clone(),
                busy.clone(),
                onregistered.clone(),
                config.clone(),
            );
            busy.set(true);
            spawn_local(async move {
                let request = Api::new(&config).register_holder(&holder);
                let result = client::send_empty(&config, &request).await;
                busy.set(false);
                match result {
                    Ok(()) => onregistered.emit(holder.did.clone()),
                    Err(e) => failure.set(Some(e.to_string())),
                }
            });
        })
    };
    let set = |state: &UseStateHandle<String>| {
        let state = state.clone();
        Callback::from(move |v: String| state.set(v))
    };

    html! {
        <Dialog
            title="Register holder"
            onclose={props.onclose.clone()}
            footer={form_footer("register-holder-form", "Register", *busy, &props.onclose)}
        >
            if let Some(message) = &*failure {
                <ErrorAlert title="Could not register holder" message={message.clone()} />
            }
            <p class="pf-v6-u-mb-md">{ "Holders registered here are approved right away." }</p>
            <form id="register-holder-form" class="pf-v6-c-form" novalidate=true onsubmit={on_submit(submit)}>
                <TextField
                    id="holder-did"
                    label="DID"
                    value={(*did).clone()}
                    oninput={set(&did)}
                    error={errors.did.clone()}
                    placeholder="did:web:example.com"
                />
                <TextField id="holder-name" label="Name" value={(*name).clone()} oninput={set(&name)} helper={AttrValue::Static("Optional display name.")} />
            </form>
        </Dialog>
    }
}
