mod config;
mod identity;
mod routes;
mod store;
mod views;

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    println!("ds-authority-governance-ui only targets web (wasm32)");
}

#[cfg(target_arch = "wasm32")]
fn main() {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));

    yew::Renderer::<app::App>::new().render();
}

/// The app shell: loads `configuration.json` once at startup, then renders
/// the masthead/nav/routed-content chrome that the 5 route views plug into.
///
/// Gated to wasm32 because it talks to `web_sys::window()` and drives a
/// browser `fetch` via `reqwest`, same as `ds-catalog-browser-ui`'s own
/// `app` module, which this loading/error-state pattern is modelled on.
#[cfg(target_arch = "wasm32")]
mod app {
    use edc_identity_hub_client::{IdentityHubClient, IdentityHubClientError, IdentityHubClientVersion};
    use patternfly_yew::prelude::*;
    use yew::platform::spawn_local;
    use yew::prelude::*;
    // `yew_router::prelude::Switch` collides with `patternfly_yew`'s own
    // (unrelated) toggle-switch `Switch` component - import it under a
    // distinct name rather than glob-importing both, same fix as
    // `patternfly-yew-quickstart` uses (`RouterSwitch`) for the analogous
    // clash with `yew_nested_router`.
    use yew_router::prelude::{use_navigator, HashRouter, Link, Switch as RouteSwitch};
    use yewdux::prelude::*;

    use crate::config::{document_origin, fetch_config};
    use crate::identity::{disconnect, fetch_userinfo, force_login_redirect};
    use crate::routes::Route;
    use crate::store::AppState;
    use crate::views::{CredentialDefinitions, Credentials, Dashboard, Holders, IdentityBootstrap};

    /// Where the page currently is in the load-configuration pipeline.
    #[derive(Clone, PartialEq)]
    enum LoadState {
        Loading,
        /// `configuration.json` could not be fetched or parsed.
        ConfigError(String),
        /// `configuration.json` and `/api/userinfo` both loaded, but the
        /// identity-api bootstrap check itself failed (network error, or a
        /// non-success response).
        BootstrapCheckError(String),
        Ready {
            /// No participant context exists yet, or its DID isn't
            /// `PUBLISHED` -- the app should open on the bootstrap wizard
            /// rather than the normal Dashboard.
            needs_bootstrap: bool,
        },
    }

    fn describe_identity_hub_error(error: IdentityHubClientError) -> String {
        match error {
            IdentityHubClientError::Reqwest(error) => error.to_string(),
            IdentityHubClientError::Response(response) => {
                format!("identity-api responded with {}", response.status())
            }
        }
    }

    /// `ParticipantContextState::ACTIVATED`'s wire value
    /// (org.eclipse.edc.participantcontext.spi.types.ParticipantContextState,
    /// eclipse-edc/Connector, confirmed at the pinned v0.18.0 tag: CREATED=100,
    /// ACTIVATED=200, DEACTIVATED=300). `Participant::state` (this crate's
    /// model) deserializes the raw int the API returns, not an enum, so the
    /// numeric literal is compared directly here rather than invented as a
    /// second, redundant enum on this side of the wire.
    const PARTICIPANT_STATE_ACTIVATED: u16 = 200;

    /// The "at startup, check if this authority is bootstrapped" gate: `true`
    /// when there's no participant context yet, or the first one isn't
    /// `ACTIVATED` yet.
    ///
    /// Deliberately does NOT call a DID-state/publish endpoint: the Issuer
    /// Service launcher's identity-api does not register did-api at all
    /// (confirmed by reading issuerservice-base-bom's build.gradle.kts --
    /// it depends on `extensions:api:identity-api:participant-context-api`
    /// but NOT `extensions:api:identity-api:did-api`, unlike the plain
    /// IdentityHub/wallet launcher's identityhub-base-bom, which has both).
    /// A previous version of this check called `get_did_state` and got a
    /// live 404 as a result. Activation IS the correct and only signal here:
    /// `DidDocumentServiceImpl` (core/identity-hub-did) listens for the
    /// `ParticipantContextUpdated` event fired by activating a participant
    /// and auto-publishes its DID as a side effect -- there is no separate
    /// "publish" step to check on this role.
    ///
    /// Only the first context is checked -- this app bootstraps exactly
    /// one identity per deployment (the `ds42-authority` case tracked by
    /// infra#84), the same assumption `AppState::selected_participant_context`
    /// already makes everywhere else in this app (a single scalar, never a
    /// list).
    async fn check_needs_bootstrap() -> Result<bool, String> {
        let origin = document_origin()
            .ok_or_else(|| "could not determine the page origin".to_string())?;
        // No API key passed here: identity-api's own `x-api-key` auth is
        // injected server-side by apisix (see Config's doc comment), never
        // by this app.
        let client = IdentityHubClient::new(
            reqwest::Client::new(),
            origin,
            None,
            IdentityHubClientVersion::V1Beta,
        );

        let participants = client
            .get_participants(0, 1)
            .await
            .map_err(describe_identity_hub_error)?;

        let Some(first) = participants.into_iter().next() else {
            return Ok(true);
        };

        Ok(first.state != PARTICIPANT_STATE_ACTIVATED)
    }

    #[function_component(App)]
    pub fn app() -> Html {
        let state = use_state(|| LoadState::Loading);
        let (_app_state, dispatch) = use_store::<AppState>();

        {
            let state = state.clone();
            use_effect_with((), move |_| {
                spawn_local(async move {
                    match fetch_config().await {
                        Ok(config) => {
                            dispatch.reduce_mut(|app_state| {
                                app_state.config = Some(config.clone());
                            });

                            match fetch_userinfo().await {
                                Ok(user) => {
                                    dispatch.reduce_mut(|app_state| {
                                        app_state.user = Some(user);
                                    });

                                    match check_needs_bootstrap().await {
                                        Ok(needs_bootstrap) => {
                                            state.set(LoadState::Ready { needs_bootstrap })
                                        }
                                        Err(message) => {
                                            state.set(LoadState::BootstrapCheckError(message))
                                        }
                                    }
                                }
                                Err(_) => {
                                    // Not authenticated (or the session
                                    // expired) -- a `fetch` redirect is
                                    // followed silently with no visible
                                    // login UI, so force a REAL top-level
                                    // navigation instead. Leaves `state`
                                    // as `Loading`: the page is about to
                                    // navigate away entirely, there's
                                    // nothing useful to render in the
                                    // meantime.
                                    force_login_redirect();
                                }
                            }
                        }
                        Err(message) => state.set(LoadState::ConfigError(message)),
                    }
                });

                || ()
            });
        }

        match &*state {
            LoadState::Loading => html!(
                <Bullseye>
                    <Spinner size={SpinnerSize::Xl} aria_label="Loading configuration" />
                </Bullseye>
            ),
            LoadState::ConfigError(message) => html!(
                <Bullseye>
                    <Alert
                        r#type={AlertType::Danger}
                        title="Could not load configuration.json"
                        inline=true
                    >
                        <p>{ message.clone() }</p>
                    </Alert>
                </Bullseye>
            ),
            LoadState::BootstrapCheckError(message) => html!(
                <Bullseye>
                    <Alert
                        r#type={AlertType::Danger}
                        title="Could not determine whether this deployment's identity is bootstrapped"
                        inline=true
                    >
                        <p>{ message.clone() }</p>
                    </Alert>
                </Bullseye>
            ),
            LoadState::Ready { needs_bootstrap } => html!(<Shell needs_bootstrap={*needs_bootstrap} />),
        }
    }

    /// The masthead + nav-sidebar + routed-content chrome, mounted only
    /// once `configuration.json` has loaded - route views can therefore
    /// assume `AppState::config` is populated.
    ///
    /// Routing is hash-based (`HashRouter`, URLs like `/#/holders`), never
    /// history/browser mode, so the built bundle can be dropped on any
    /// static host/CDN with zero server-side fallback-rule configuration.
    #[derive(Properties, PartialEq)]
    struct ShellProps {
        /// Forwarded straight to `BootstrapRedirect`, rendered inside the
        /// `HashRouter` this component owns -- see that component's own
        /// doc comment for why the redirect can't just happen here.
        needs_bootstrap: bool,
    }

    #[function_component(Shell)]
    fn shell(props: &ShellProps) -> Html {
        let brand = html!(
            <Title level={Level::H3} size={Size::XXLarge}>
                { "DS Authority Governance Console" }
            </Title>
        );

        let sidebar = html_nested! {
            <PageSidebar>
                <Nav>
                    <NavList>
                        { nav_link(Route::Dashboard, "Dashboard") }
                        { nav_link(Route::CredentialDefinitions, "Credential Definitions") }
                        { nav_link(Route::Holders, "Holders") }
                        { nav_link(Route::Credentials, "Credentials") }
                        { nav_link(Route::IdentityBootstrap, "Identity Bootstrap") }
                    </NavList>
                </Nav>
            </PageSidebar>
        };

        let tools = html!(<IdentityBadge />);
        let needs_bootstrap = props.needs_bootstrap;

        html!(
            <HashRouter>
                <BootstrapRedirect {needs_bootstrap} />
                <Page {brand} {sidebar} {tools} full_height=true>
                    <RouteSwitch<Route> render={switch} />
                </Page>
            </HashRouter>
        )
    }

    /// Performs the actual "redirect the user to the wizard" the startup
    /// check calls for: a real `yew_router` navigation (`Navigator::push`)
    /// to `Route::IdentityBootstrap`, run once on mount when
    /// `needs_bootstrap` is `true`.
    ///
    /// Chosen over rendering `IdentityBootstrapWizard` full-page in place
    /// of `Shell` (the other option this component could have taken):
    /// pushing a real route changes the URL hash to `#/identity-bootstrap`
    /// -- reloading the page, or sharing the link, lands back on the
    /// wizard exactly the way a normal redirect would, the nav sidebar
    /// highlights the right entry, and `Route::IdentityBootstrap`'s own
    /// view (`IdentityBootstrap`) is reused unchanged. A full-page swap
    /// with no route/URL change would look like a redirect but not behave
    /// like one under a reload or a shared link.
    ///
    /// Has to be its own component, rendered *inside* `<HashRouter>`
    /// (`Shell` renders it as a sibling of `<Page>`, both inside the
    /// router it owns): `use_navigator` reads a context `HashRouter`
    /// provides to its descendants, which is not yet available to `Shell`
    /// itself at the point `Shell`'s own body runs (a component's hooks
    /// run in the context of where it's mounted, not the context of the
    /// tree it's about to render).
    #[derive(Properties, PartialEq)]
    struct BootstrapRedirectProps {
        needs_bootstrap: bool,
    }

    #[function_component(BootstrapRedirect)]
    fn bootstrap_redirect(props: &BootstrapRedirectProps) -> Html {
        let navigator = use_navigator();
        let needs_bootstrap = props.needs_bootstrap;

        use_effect_with(needs_bootstrap, move |needs_bootstrap| {
            if *needs_bootstrap {
                if let Some(navigator) = navigator {
                    navigator.push(&Route::IdentityBootstrap);
                }
            }
            || ()
        });

        html!()
    }

    /// The top-right identity control (`Page::tools`, patternfly-yew's own
    /// masthead slot for exactly this -- confirmed in its source, not
    /// guessed). Renders nothing until `AppState.user` is populated (it
    /// always is by the time `Shell` mounts, since `App` only renders
    /// `Shell` after both configuration.json and /api/userinfo have
    /// loaded -- see `LoadState::Ready` above -- but reading `AppState`
    /// directly here, rather than threading it through as a prop, means
    /// this stays correct even if that load-order assumption ever
    /// changes).
    #[function_component(IdentityBadge)]
    fn identity_badge() -> Html {
        let (app_state, _dispatch) = use_store::<AppState>();

        let Some(user) = app_state.user.clone() else {
            return html!();
        };

        let on_disconnect = Callback::from(|_: ()| disconnect());

        html!(
            <Dropdown
                icon={html!(Icon::User)}
                variant={MenuToggleVariant::Plain}
                aria_label="Signed-in user"
            >
                <Raw>
                    <div class="pf-v6-u-p-md">
                        <div class="pf-v6-u-font-weight-bold">{ user.display_name() }</div>
                        if let Some(email) = &user.email {
                            // No plain "muted text" utility class exists in
                            // PatternFly v6's shipped CSS (confirmed by
                            // grepping the actual built bundle) -- v6 moved
                            // this to design tokens instead. Use the real
                            // token (--pf-t--global--text--color--subtle)
                            // directly rather than guess at a utility class
                            // name that doesn't exist.
                            <div style="color: var(--pf-t--global--text--color--subtle);">
                                { email }
                            </div>
                        }
                    </div>
                </Raw>
                <ListDivider />
                <MenuAction danger=true onclick={on_disconnect}>
                    { "Disconnect" }
                </MenuAction>
            </Dropdown>
        )
    }

    /// One nav-sidebar entry, wired to `yew_router`'s `Link` (not
    /// `patternfly_yew`'s `NavRouterItem`, which targets the separate
    /// `yew_nested_router` crate this app doesn't use) so it navigates
    /// within the `HashRouter` without a full page reload.
    fn nav_link(to: Route, label: &'static str) -> Html {
        html! (
            <li class="pf-v6-c-nav__item">
                <Link<Route> classes={classes!("pf-v6-c-nav__link")} {to}>
                    { label }
                </Link<Route>>
            </li>
        )
    }

    fn switch(route: Route) -> Html {
        match route {
            Route::Dashboard => html!(<Dashboard />),
            Route::CredentialDefinitions => html!(<CredentialDefinitions />),
            Route::Holders => html!(<Holders />),
            Route::Credentials => html!(<Credentials />),
            Route::IdentityBootstrap => html!(<IdentityBootstrap />),
        }
    }
}
