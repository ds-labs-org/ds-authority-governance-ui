//! Hooks shared by the route views: runtime config access, API-backed
//! loading with silent reloads, and polling.

use std::cell::{Cell, RefCell};
use std::future::Future;
use std::rc::Rc;
use std::time::Duration;

use yew::platform::spawn_local;
use yew::prelude::*;
use yewdux::prelude::*;

use crate::api::ApiError;
use crate::config::Config;
use crate::store::AppState;

/// The configuration loaded at startup (the shell only mounts the routes
/// once it is there).
#[hook]
pub fn use_config() -> Config {
    let (state, _) = use_store::<AppState>();
    state.config.clone().unwrap_or_default()
}

/// State of one API-backed resource.
#[derive(Clone, Debug, PartialEq)]
pub enum Load<T> {
    /// First load in progress (nothing to show yet).
    Loading,
    Ready(T),
    /// The last load failed; the message is user-facing.
    Failed(String),
}

impl<T> Load<T> {
    pub fn ready(&self) -> Option<&T> {
        match self {
            Load::Ready(value) => Some(value),
            _ => None,
        }
    }
}

/// Loads a resource with `fetch` on mount and whenever the returned
/// callback is emitted. Reloads keep showing the previous data until the
/// new result arrives (no spinner flicker); a response that arrives after
/// a newer reload started is dropped.
#[hook]
pub fn use_load<T, F, Fut>(fetch: F) -> (Load<T>, Callback<()>)
where
    T: Clone + PartialEq + 'static,
    F: FnOnce(Config) -> Fut + 'static,
    Fut: Future<Output = Result<T, ApiError>> + 'static,
{
    let config = use_config();
    let state = use_state(|| Load::Loading);
    let generation = use_state(|| 0u32);
    let counter = use_mut_ref(|| 0u32);
    let latest = use_mut_ref(|| 0u32);

    {
        let state = state.clone();
        let latest: Rc<RefCell<u32>> = latest.clone();
        use_effect_with(
            (*generation, config.clone()),
            move |(generation, config)| {
                let generation = *generation;
                *latest.borrow_mut() = generation;
                let future = fetch(config.clone());
                spawn_local(async move {
                    let result = future.await;
                    if *latest.borrow() == generation {
                        state.set(match result {
                            Ok(value) => Load::Ready(value),
                            Err(error) => Load::Failed(error.to_string()),
                        });
                    }
                });
                || ()
            },
        );
    }

    let reload = {
        let generation = generation.clone();
        Callback::from(move |()| {
            let next = {
                let mut counter = counter.borrow_mut();
                *counter += 1;
                *counter
            };
            generation.set(next);
        })
    };

    ((*state).clone(), reload)
}

/// Emits `tick` every `period` while `active` is true.
#[hook]
pub fn use_polling(active: bool, period: Duration, tick: Callback<()>) {
    let tick_ref = use_mut_ref(|| tick.clone());
    *tick_ref.borrow_mut() = tick;
    use_effect_with(active, move |active| {
        let alive = Rc::new(Cell::new(*active));
        if *active {
            let alive = alive.clone();
            spawn_local(async move {
                loop {
                    yew::platform::time::sleep(period).await;
                    if !alive.get() {
                        break;
                    }
                    let tick = tick_ref.borrow().clone();
                    tick.emit(());
                }
            });
        }
        move || alive.set(false)
    });
}
