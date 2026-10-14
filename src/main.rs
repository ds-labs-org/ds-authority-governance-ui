// The app itself only runs on wasm32; on the host only the unit tests use
// these modules, so unused-item lints there are noise.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

mod api;
mod components;
mod config;
mod format;
mod hooks;
mod routes;
mod store;
mod validation;
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
/// the masthead/nav/routed-content chrome that the 6 route views plug into.
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
    use yew_router::prelude::{HashRouter, Link, Switch as RouteSwitch, use_route};
    use yewdux::prelude::*;

    use crate::components::ErrorAlert;
    use crate::config::fetch_config;
    use crate::routes::Route;
    use crate::store::AppState;
    use crate::views::{
        CredentialDefinitions, Credentials, Dashboard, Holders, IdentityBootstrap, TrustedIssuers,
    };

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
                    let Some(origin) = current_origin() else {
                        state.set(LoadState::ConfigError(
                            "could not determine the page origin".to_string(),
                        ));
                        return;
                    };

                    match fetch_config(&origin).await {
                        Ok(config) => {
                            dispatch.reduce_mut(|app_state| {
                                app_state.config = Some(config);
                            });
                            state.set(LoadState::Ready);
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
                    <ErrorAlert title="Could not load configuration.json" message={message.clone()} />
                </Bullseye>
            ),
            LoadState::Ready => html!(<Shell />),
        }
    }

    fn current_origin() -> Option<String> {
        web_sys::window()?.location().origin().ok()
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
        html!(
            <HashRouter>
                <Chrome />
            </HashRouter>
        )
    }

    /// PatternFly's sidebar only sits beside the content from the `xl`
    /// breakpoint up; below it the sidebar overlays the page.
    const SIDEBAR_BREAKPOINT_PX: f64 = 1200.0;

    fn is_wide() -> bool {
        web_sys::window()
            .and_then(|w| w.inner_width().ok())
            .and_then(|w| w.as_f64())
            .is_none_or(|w| w >= SIDEBAR_BREAKPOINT_PX)
    }

    /// Masthead with a nav toggle, sidebar and routed content. The sidebar
    /// starts collapsed on narrow screens and closes again after a nav
    /// click there, so it never sits over the page the user navigated to.
    #[function_component(Chrome)]
    fn chrome() -> Html {
        let open = use_state(is_wide);
        let current = use_route::<Route>();

        let toggle = {
            let open = open.clone();
            Callback::from(move |_: MouseEvent| open.set(!*open))
        };
        let onnavigate = {
            let open = open.clone();
            Callback::from(move |_: MouseEvent| {
                if !is_wide() {
                    open.set(false);
                }
            })
        };
        let nav_link = |to: Route, label: &'static str| {
            let is_current = current.as_ref() == Some(&to);
            html! {
                <li class="pf-v6-c-nav__item" onclick={onnavigate.clone()}>
                    <Link<Route>
                        classes={classes!("pf-v6-c-nav__link", is_current.then_some("pf-m-current"))}
                        {to}
                    >
                        { label }
                    </Link<Route>>
                </li>
            }
        };

        html! {
            <div class="pf-v6-c-page">
                <header class="pf-v6-c-masthead">
                    <div class="pf-v6-c-masthead__main">
                        <span class="pf-v6-c-masthead__toggle">
                            <button
                                class="pf-v6-c-button pf-m-plain"
                                type="button"
                                aria-label="Global navigation"
                                aria-expanded={open.to_string()}
                                aria-controls="primary-sidebar"
                                onclick={toggle}
                            >
                                <i class="fas fa-bars" aria-hidden="true" />
                            </button>
                        </span>
                        <div class="pf-v6-c-masthead__brand">
                            <Title level={Level::H3} size={Size::XLarge}>
                                { "DS Authority Governance Console" }
                            </Title>
                        </div>
                    </div>
                </header>
                <div
                    id="primary-sidebar"
                    class={classes!("pf-v6-c-page__sidebar", if *open { "pf-m-expanded" } else { "pf-m-collapsed" })}
                    aria-hidden={(!*open).to_string()}
                    inert={!*open}
                >
                    <div class="pf-v6-c-page__sidebar-body">
                        <Nav>
                            <NavList>
                                { nav_link(Route::Dashboard, "Dashboard") }
                                { nav_link(Route::CredentialDefinitions, "Credential Definitions") }
                                { nav_link(Route::Holders, "Holders") }
                                { nav_link(Route::Credentials, "Credentials") }
                                { nav_link(Route::TrustedIssuers, "Trusted Issuers") }
                                { nav_link(Route::IdentityBootstrap, "Identity Bootstrap") }
                            </NavList>
                        </Nav>
                    </div>
                </div>
                <div class="pf-v6-c-page__main-container pf-m-fill">
                    <main class="pf-v6-c-page__main" tabindex="-1">
                        <RouteSwitch<Route> render={switch} />
                    </main>
                </div>
            </div>
        }
    }

    fn switch(route: Route) -> Html {
        match route {
            Route::Dashboard => html!(<Dashboard />),
            Route::CredentialDefinitions => html!(<CredentialDefinitions />),
            Route::Holders => html!(<Holders />),
            Route::Credentials => html!(<Credentials />),
            Route::TrustedIssuers => html!(<TrustedIssuers />),
            Route::IdentityBootstrap => html!(<IdentityBootstrap />),
        }
    }
}
