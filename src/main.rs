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
    use patternfly_yew::prelude::*;
    use yew::platform::spawn_local;
    use yew::prelude::*;
    // `yew_router::prelude::Switch` collides with `patternfly_yew`'s own
    // (unrelated) toggle-switch `Switch` component - import it under a
    // distinct name rather than glob-importing both, same fix as
    // `patternfly-yew-quickstart` uses (`RouterSwitch`) for the analogous
    // clash with `yew_nested_router`.
    use yew_router::prelude::{HashRouter, Link, Switch as RouteSwitch};
    use yewdux::prelude::*;

    use crate::config::fetch_config;
    use crate::identity::{fetch_userinfo, force_login_redirect};
    use crate::routes::Route;
    use crate::store::AppState;
    use crate::views::{CredentialDefinitions, Credentials, Dashboard, Holders, IdentityBootstrap};

    /// Where the page currently is in the load-configuration pipeline.
    #[derive(Clone, PartialEq)]
    enum LoadState {
        Loading,
        /// `configuration.json` could not be fetched or parsed.
        ConfigError(String),
        Ready,
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
                                app_state.config = Some(config);
                            });

                            match fetch_userinfo().await {
                                Ok(user) => {
                                    dispatch.reduce_mut(|app_state| {
                                        app_state.user = Some(user);
                                    });
                                    state.set(LoadState::Ready);
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
            LoadState::Ready => html!(<Shell />),
        }
    }

    /// The masthead + nav-sidebar + routed-content chrome, mounted only
    /// once `configuration.json` has loaded - route views can therefore
    /// assume `AppState::config` is populated.
    ///
    /// Routing is hash-based (`HashRouter`, URLs like `/#/holders`), never
    /// history/browser mode, so the built bundle can be dropped on any
    /// static host/CDN with zero server-side fallback-rule configuration.
    #[function_component(Shell)]
    fn shell() -> Html {
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

        html!(
            <HashRouter>
                <Page {brand} {sidebar} full_height=true>
                    <RouteSwitch<Route> render={switch} />
                </Page>
            </HashRouter>
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
