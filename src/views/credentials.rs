//! `/credentials`: issued credentials (with revocation) and issuance
//! processes. Polls every few seconds while a process is in flight.

use std::time::Duration;

use patternfly_yew::prelude::*;
use yew::platform::spawn_local;
use yew::prelude::*;

use crate::api::{
    Api, CredentialStatus, IssuanceProcess, IssuedCredential, ProcessState, QuerySpec, client,
};
use crate::components::*;
use crate::format::{timestamp, timestamp_or};
use crate::hooks::{Flight, InFlight, Load, Show, Shown, use_config, use_load, use_polling};

const ISSUED_COLUMNS: [&str; 7] = ["ID", "Type", "Holder", "Issued", "Expires", "Status", ""];
const PROCESS_COLUMNS: [&str; 5] = ["ID", "Holder", "Credential definitions", "State", "Updated"];
/// While a delivery is in flight (APPROVED) the state changes within
/// seconds; SUBMITTED waits on a human, so only a slow background check.
const POLL_DELIVERY: Duration = Duration::from_secs(3);
const POLL_WAITING: Duration = Duration::from_secs(30);

/// How often to refresh given the current process states, if at all.
fn poll_period(states: impl IntoIterator<Item = ProcessState>) -> Option<Duration> {
    let mut period = None;
    for state in states {
        match state {
            ProcessState::Approved => return Some(POLL_DELIVERY),
            ProcessState::Submitted => period = Some(POLL_WAITING),
            _ => {}
        }
    }
    period
}

fn credential_status_label(status: CredentialStatus) -> Html {
    let color = match status {
        CredentialStatus::Active => Color::Green,
        CredentialStatus::Revoked => Color::Red,
        CredentialStatus::Unknown => Color::Grey,
    };
    status_label(status.label(), color)
}

fn process_state_label(state: ProcessState) -> Html {
    let color = match state {
        ProcessState::Submitted => Color::Orange,
        ProcessState::Approved => Color::Blue,
        ProcessState::Delivered => Color::Green,
        ProcessState::Errored | ProcessState::Rejected => Color::Red,
        ProcessState::Unknown => Color::Grey,
    };
    status_label(state.as_str(), color)
}

#[function_component(Credentials)]
pub fn credentials() -> Html {
    let config = use_config();
    let (issued, reload_issued) = use_load(|config| async move {
        let request = Api::new(&config).query_credentials(&QuerySpec::all_newest_first("issuedAt"));
        client::send_json::<Vec<IssuedCredential>>(&config, &request).await
    });
    let (processes, reload_processes) = use_load(|config| async move {
        let request =
            Api::new(&config).query_issuance_processes(&QuerySpec::all_newest_first("createdAt"));
        client::send_json::<Vec<IssuanceProcess>>(&config, &request).await
    });
    let revoking = use_reducer(Shown::<Option<IssuedCredential>>::default);
    let busy = use_reducer(InFlight::default);
    let feedback = use_state(|| None::<Feedback>);

    let reload_all = {
        let (a, b) = (reload_issued.clone(), reload_processes.clone());
        Callback::from(move |()| {
            a.emit(());
            b.emit(());
        })
    };
    let period = processes
        .ready()
        .and_then(|list| poll_period(list.iter().map(|p| p.state)));
    let delivering = period == Some(POLL_DELIVERY);
    use_polling(period, reload_all.clone());

    let close = {
        let revoking = revoking.clone();
        Callback::from(move |()| revoking.dispatch(Show::Close))
    };
    let onrevoke = {
        let (revoking, busy, feedback, reload, config) = (
            revoking.clone(),
            busy.clone(),
            feedback.clone(),
            reload_issued.clone(),
            config.clone(),
        );
        Callback::from(move |credential: IssuedCredential| {
            let (revoking, busy, feedback, reload, config) = (
                revoking.clone(),
                busy.clone(),
                feedback.clone(),
                reload.clone(),
                config.clone(),
            );
            if busy.has(&credential.id) {
                return;
            }
            busy.dispatch(Flight::Start(credential.id.clone()));
            spawn_local(async move {
                let request = Api::new(&config).revoke_credential(&credential.id);
                let result = client::send_empty(&config, &request).await;
                busy.dispatch(Flight::Done(credential.id.clone()));
                revoking.dispatch(Show::CloseIf(Some(credential.clone())));
                feedback.set(Some(match result {
                    Ok(()) => Feedback::Success(format!(
                        "Revoked {}. The status list now marks it revoked.",
                        credential.id
                    )),
                    Err(e) => Feedback::error("Could not revoke credential", e),
                }));
                reload.emit(());
            });
        })
    };

    let issued_body = match &issued {
        Load::Loading => html!(<Loading label="Loading issued credentials" />),
        Load::Failed(message) => {
            html!(<ErrorAlert title="Could not load issued credentials" message={message.clone()} />)
        }
        Load::Ready(list) => issued_table(list, &revoking, &busy),
    };
    let processes_body = match &processes {
        Load::Loading => html!(<Loading label="Loading issuance processes" />),
        Load::Failed(message) => {
            html!(<ErrorAlert title="Could not load issuance processes" message={message.clone()} />)
        }
        Load::Ready(list) => process_table(list),
    };

    let dialog = match &revoking.0 {
        None => html!(),
        Some(credential) => {
            let target = credential.clone();
            html! {
                <ConfirmDialog
                    title="Revoke credential"
                    confirm_label="Revoke"
                    busy={busy.has(&credential.id)}
                    onclose={close}
                    onconfirm={onrevoke.reform(move |()| target.clone())}
                >
                    <p>
                        { "Revoke " }<strong>{ credential.credential_type.clone() }</strong>
                        { " " }<code class="pf-v6-u-text-break-word">{ credential.id.clone() }</code>
                        { " held by " }<strong class="pf-v6-u-text-break-word">{ credential.holder_did.clone() }</strong>{ "?" }
                    </p>
                    <p class="pf-v6-u-mt-sm">
                        { "Verifiers checking the revocation status list will reject it. This cannot be undone." }
                    </p>
                </ConfirmDialog>
            }
        }
    };

    html! {
        <>
            <PageHeader
                title="Credentials"
                description="Credentials this issuer signed and delivered, and the issuance processes behind them."
            />
            <PageSection>
                <FeedbackAlert feedback={(*feedback).clone()} onclose={{ let f = feedback.clone(); Callback::from(move |()| f.set(None)) }} />
                <div class="pf-v6-l-flex pf-m-align-items-center pf-m-gap-sm pf-v6-u-mb-md">
                    <PfButton variant={ButtonVariant::Secondary} label="Refresh" icon={Icon::SyncAlt} onclick={reload_all.reform(|_| ())} />
                    if delivering {
                        <span class="pf-v6-u-color-200">{ "Delivery in progress, refreshing automatically every few seconds." }</span>
                    }
                </div>
                <section class="pf-v6-u-mb-xl" aria-labelledby="issued-title">
                    <Title level={Level::H2} size={Size::Large}><span id="issued-title">{ "Issued credentials" }</span></Title>
                    { issued_body }
                </section>
                <section aria-labelledby="processes-title">
                    <Title level={Level::H2} size={Size::Large}><span id="processes-title">{ "Issuance processes" }</span></Title>
                    { processes_body }
                </section>
            </PageSection>
            { dialog }
        </>
    }
}

fn issued_table(
    list: &[IssuedCredential],
    revoking: &UseReducerHandle<Shown<Option<IssuedCredential>>>,
    busy: &InFlight,
) -> Html {
    let rows = if list.is_empty() {
        empty_row(
            ISSUED_COLUMNS.len(),
            "No issued credentials",
            html!("Credentials appear here once delivered to a holder."),
        )
    } else {
        list.iter()
            .map(|credential| {
                let revoke = {
                    let (revoking, credential) = (revoking.clone(), credential.clone());
                    Callback::from(move |_| revoking.dispatch(Show::Open(Some(credential.clone()))))
                };
                let revoked = credential.status == CredentialStatus::Revoked;
                html! {
                    <tr class="pf-v6-c-table__tr" key={credential.id.clone()}>
                        { long_cell("ID", &credential.id) }
                        { cell("Type", credential.credential_type.clone()) }
                        { long_cell("Holder", &credential.holder_did) }
                        { cell("Issued", timestamp(credential.issued_at)) }
                        { cell("Expires", timestamp_or(credential.expires_at, "Never")) }
                        { cell("Status", credential_status_label(credential.status)) }
                        <td class="pf-v6-c-table__td pf-v6-c-table__action" data-label="Actions">
                            <PfButton size={ButtonSize::Small} variant={ButtonVariant::DangerSecondary} label="Revoke" onclick={revoke} disabled={revoked || busy.has(&credential.id)} />
                        </td>
                    </tr>
                }
            })
            .collect::<Html>()
    };
    html! {
        <table class="pf-v6-c-table pf-m-grid-md" aria-label="Issued credentials">
            { header_row(&ISSUED_COLUMNS) }
            <tbody class="pf-v6-c-table__tbody">{ rows }</tbody>
        </table>
    }
}

fn process_table(list: &[IssuanceProcess]) -> Html {
    let rows = if list.is_empty() {
        empty_row(
            PROCESS_COLUMNS.len(),
            "No issuance processes",
            html!("A process starts when a holder sends a credential request."),
        )
    } else {
        list.iter()
            .map(|process| {
                html! {
                    <tr class="pf-v6-c-table__tr" key={process.id.clone()}>
                        { long_cell("ID", &process.id) }
                        { long_cell("Holder", &process.holder_did) }
                        { cell("Credential definitions", process.credential_definitions.join(", ")) }
                        <td class="pf-v6-c-table__td" data-label="State">
                            { process_state_label(process.state) }
                            if let Some(detail) = &process.error_detail {
                                <div class="pf-v6-u-font-size-sm pf-v6-u-color-200">{ detail.clone() }</div>
                            }
                        </td>
                        { cell("Updated", timestamp(process.state_timestamp.max(process.created_at))) }
                    </tr>
                }
            })
            .collect::<Html>()
    };
    html! {
        <table class="pf-v6-c-table pf-m-grid-md" aria-label="Issuance processes">
            { header_row(&PROCESS_COLUMNS) }
            <tbody class="pf-v6-c-table__tbody">{ rows }</tbody>
        </table>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polls_fast_only_while_a_delivery_is_in_flight() {
        use ProcessState::*;
        assert_eq!(
            poll_period([Delivered, Approved, Submitted]),
            Some(POLL_DELIVERY)
        );
        assert_eq!(poll_period([Submitted, Delivered]), Some(POLL_WAITING));
        assert_eq!(poll_period([Delivered, Rejected, Errored]), None);
        assert_eq!(poll_period([]), None);
    }
}
