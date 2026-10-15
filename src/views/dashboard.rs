//! `/`: counts at a glance, the pending-approval call to action, and this
//! issuer's own standing in the trusted issuers registry.

use patternfly_yew::prelude::*;
use yew::prelude::*;
use yew_router::prelude::Link;

use crate::api::{
    Api, ApiError, CredentialDefinition, CredentialStatus, Holder, IssuanceProcess,
    IssuedCredential, Participant, QuerySpec, TrustDecision, client, query::MAX_LIMIT,
};
use crate::components::*;
use crate::config::Config;
use crate::hooks::{Load, use_load};
use crate::routes::Route;

async fn query<T: serde::de::DeserializeOwned>(
    config: &Config,
    request: crate::api::ApiRequest,
) -> Result<Vec<T>, ApiError> {
    client::send_json::<Vec<T>>(config, &request).await
}

/// `n`, or `n+` when the hub's page cap was hit.
fn count(n: usize) -> String {
    if n >= MAX_LIMIT as usize {
        format!("{n}+")
    } else {
        n.to_string()
    }
}

#[derive(Clone, PartialEq)]
struct Issued {
    total: usize,
    active: usize,
    revoked: usize,
}

#[derive(Clone, PartialEq)]
struct TrustStatus {
    did: String,
    /// `(credential type, decision or error message)`.
    types: Vec<(String, Result<TrustDecision, String>)>,
}

#[function_component(Dashboard)]
pub fn dashboard() -> Html {
    let all = || QuerySpec::new().limit(MAX_LIMIT);
    let (definitions, _) = use_load(move |c| async move {
        query::<CredentialDefinition>(&c, Api::new(&c).query_credential_definitions(&all())).await
    });
    let (pending, _) = use_load(move |c| async move {
        let q = all().filter("status", "pending");
        query::<Holder>(&c, Api::new(&c).query_holders(&q))
            .await
            .map(|v| v.len())
    });
    let (in_flight, _) = use_load(move |c| async move {
        let api = Api::new(&c);
        let submitted = query::<IssuanceProcess>(
            &c,
            api.query_issuance_processes(&all().filter("state", "SUBMITTED")),
        );
        let approved = query::<IssuanceProcess>(
            &c,
            api.query_issuance_processes(&all().filter("state", "APPROVED")),
        );
        let (submitted, approved) = (submitted.await?, approved.await?);
        Ok((submitted.len(), approved.len()))
    });
    let (issued, _) = use_load(move |c| async move {
        let list = query::<IssuedCredential>(&c, Api::new(&c).query_credentials(&all())).await?;
        let by = |s| list.iter().filter(|c| c.status == s).count();
        Ok(Issued {
            total: list.len(),
            active: by(CredentialStatus::Active),
            revoked: by(CredentialStatus::Revoked),
        })
    });
    let (trust, _) = use_load(move |c| async move {
        let api = Api::new(&c);
        let participant: Participant = client::send_json(&c, &api.participant()).await?;
        let definitions =
            query::<CredentialDefinition>(&c, api.query_credential_definitions(&all())).await?;
        let mut types: Vec<String> = definitions.into_iter().map(|d| d.credential_type).collect();
        types.sort();
        types.dedup();
        let mut checked = Vec::with_capacity(types.len());
        for t in types {
            let decision =
                client::send_json::<TrustDecision>(&c, &api.check_trust(&participant.did, &t))
                    .await
                    .map_err(|e| e.to_string());
            checked.push((t, decision));
        }
        Ok(TrustStatus {
            did: participant.did,
            types: checked,
        })
    });

    let pending_count = pending.ready().copied();

    html! {
        <>
            <PageHeader title="Dashboard" description="State of this issuer at a glance." />
            <PageSection>
                if pending_count.is_some_and(|n| n > 0) {
                    <div class="pf-v6-c-alert pf-m-warning pf-m-inline pf-v6-u-mb-md">
                        <div class="pf-v6-c-alert__icon">{ Icon::ExclamationTriangle }</div>
                        <p class="pf-v6-c-alert__title">
                            { format!("{} holder{} waiting for approval.", pending_count.unwrap_or(0), if pending_count == Some(1) { " is" } else { "s are" }) }
                        </p>
                        <div class="pf-v6-c-alert__action-group">
                            <Link<Route> to={Route::Holders} classes={classes!("pf-v6-c-button", "pf-m-primary")}>
                                { "Review pending approvals" }
                            </Link<Route>>
                        </div>
                    </div>
                }
                <div class="pf-v6-l-gallery pf-m-gutter" style="--pf-v6-l-gallery--GridTemplateColumns--min: 15rem;">
                    { count_card("Credential definitions", &definitions.clone().map_ready(|d| count(d.len())), html!(
                        <Link<Route> to={Route::CredentialDefinitions}>{ "Manage definitions" }</Link<Route>>
                    )) }
                    { count_card("Pending holders", &pending.clone().map_ready(|n| count(*n)), html!(
                        <Link<Route> to={Route::Holders}>{ "Open holders" }</Link<Route>>
                    )) }
                    { count_card("Issuance processes in flight", &in_flight.clone().map_ready(|(s, a)| count(s + a)), match in_flight.ready() {
                        Some((s, a)) => html!(<span class="pf-v6-u-color-200">{ format!("{s} submitted, {a} approved") }</span>),
                        None => html!(),
                    }) }
                    { count_card("Issued credentials", &issued.clone().map_ready(|i| count(i.total)), match issued.ready() {
                        Some(i) => html!(
                            <span class="pf-v6-l-flex pf-m-gap-sm">
                                <Label label={format!("{} active", i.active)} color={Color::Green} compact=true />
                                <Label label={format!("{} revoked", i.revoked)} color={Color::Red} compact=true />
                            </span>
                        ),
                        None => html!(),
                    }) }
                </div>
            </PageSection>
            <PageSection>
                <section class="pf-v6-c-card" aria-labelledby="trust-title">
                    <div class="pf-v6-c-card__title">
                        <h2 class="pf-v6-c-card__title-text" id="trust-title">{ "Trust status in the registry" }</h2>
                    </div>
                    <div class="pf-v6-c-card__body">{ trust_body(&trust) }</div>
                    <div class="pf-v6-c-card__footer">
                        <Link<Route> to={Route::TrustedIssuers}>{ "Open trusted issuers" }</Link<Route>>
                    </div>
                </section>
            </PageSection>
        </>
    }
}

trait MapReady<T> {
    fn map_ready<U>(self, f: impl FnOnce(&T) -> U) -> Load<U>;
}

impl<T> MapReady<T> for Load<T> {
    fn map_ready<U>(self, f: impl FnOnce(&T) -> U) -> Load<U> {
        match self {
            Load::Loading => Load::Loading,
            Load::Ready(v) => Load::Ready(f(&v)),
            Load::Failed(e) => Load::Failed(e),
        }
    }
}

fn count_card(title: &'static str, value: &Load<String>, footer: Html) -> Html {
    let id = format!("card-{}", title.to_lowercase().replace(' ', "-"));
    let body = match value {
        Load::Loading => html!(<Loading label={format!("Loading {}", title.to_lowercase())} />),
        Load::Failed(message) => {
            html!(<ErrorAlert title={format!("Could not load {}", title.to_lowercase())} message={message.clone()} />)
        }
        Load::Ready(n) => {
            html!(<p class="pf-v6-u-font-size-4xl pf-v6-u-font-weight-bold" data-testid="count">{ n.clone() }</p>)
        }
    };
    html! {
        <section class="pf-v6-c-card" aria-label={title} aria-describedby={id.clone()}>
            <div class="pf-v6-c-card__title">
                <h2 class="pf-v6-c-card__title-text" id={id}>{ title }</h2>
            </div>
            <div class="pf-v6-c-card__body">{ body }</div>
            <div class="pf-v6-c-card__footer">{ footer }</div>
        </section>
    }
}

fn trust_body(trust: &Load<TrustStatus>) -> Html {
    match trust {
        Load::Loading => html!(<Loading label="Checking trust status" />),
        Load::Failed(message) => html!(
            <InlineAlert kind={Notice::Warning} title="Could not determine the trust status">
                <p>{ message.clone() }</p>
            </InlineAlert>
        ),
        Load::Ready(status) => html! {
            <>
                <p class="pf-v6-u-mb-sm">
                    { "This issuer: " }<code class="pf-v6-u-text-break-word">{ status.did.clone() }</code>
                </p>
                if status.types.is_empty() {
                    <p class="pf-v6-u-color-200">{ "No credential definitions yet, so nothing to check." }</p>
                } else {
                    <ul class="pf-v6-c-list pf-m-plain" aria-label="Trust per credential type">
                        { for status.types.iter().map(|(t, decision)| {
                            let (label, color, detail) = match decision {
                                Ok(d) if d.trusted => ("Trusted", Color::Green, format!("Chain: {}", d.chain.join(" -> "))),
                                Ok(d) => ("Not trusted", Color::Red, d.reason.clone().unwrap_or_default()),
                                Err(e) => ("Unknown", Color::Grey, e.clone()),
                            };
                            html! {
                                <li class="pf-v6-u-mb-xs">
                                    <strong>{ t.clone() }</strong>{ " " }
                                    <span title={detail.clone()}><Label label={label} {color} compact=true /></span>
                                    if !detail.is_empty() {
                                        <span class="pf-v6-u-ml-sm pf-v6-u-color-200 pf-v6-u-font-size-sm">{ detail }</span>
                                    }
                                </li>
                            }
                        }) }
                    </ul>
                }
            </>
        },
    }
}
