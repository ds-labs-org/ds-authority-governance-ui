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

/// Emits `tick` every `period`; `None` stops polling.
#[hook]
pub fn use_polling(period: Option<Duration>, tick: Callback<()>) {
    let tick_ref = use_mut_ref(|| tick.clone());
    *tick_ref.borrow_mut() = tick;
    use_effect_with(period, move |period| {
        let alive = Rc::new(Cell::new(period.is_some()));
        if let Some(period) = *period {
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

/// Which dialog a page shows. `T::default()` means none. Reducer-based so
/// async completions decide against the *current* state, not the one
/// captured when they started.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Shown<T>(pub T);

pub enum Show<T> {
    Open(T),
    Close,
    /// Close only if `T` is still the dialog on screen (so an action's
    /// completion never closes a dialog the user opened meanwhile).
    CloseIf(T),
}

impl<T: Clone + Default + PartialEq> Reducible for Shown<T> {
    type Action = Show<T>;

    fn reduce(self: Rc<Self>, action: Self::Action) -> Rc<Self> {
        match action {
            Show::Open(t) => Rc::new(Shown(t)),
            Show::Close => Rc::new(Shown(T::default())),
            Show::CloseIf(t) if self.0 == t => Rc::new(Shown(T::default())),
            Show::CloseIf(_) => self,
        }
    }
}

/// Keys (ids, DIDs) with a write in flight.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InFlight(pub std::collections::BTreeSet<String>);

pub enum Flight {
    Start(String),
    Done(String),
}

impl InFlight {
    pub fn has(&self, key: &str) -> bool {
        self.0.contains(key)
    }
}

impl Reducible for InFlight {
    type Action = Flight;

    fn reduce(self: Rc<Self>, action: Self::Action) -> Rc<Self> {
        let mut set = self.0.clone();
        match action {
            Flight::Start(k) => set.insert(k),
            Flight::Done(k) => set.remove(&k),
        };
        Rc::new(InFlight(set))
    }
}

/// A counter that only goes up; `bump` is safe to call from async code.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Revision(pub u32);

impl Reducible for Revision {
    type Action = ();

    fn reduce(self: Rc<Self>, _: ()) -> Rc<Self> {
        Rc::new(Revision(self.0.wrapping_add(1)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn close_if_only_closes_the_matching_dialog() {
        let shown = Rc::new(Shown(Some(1)));
        assert_eq!(shown.clone().reduce(Show::CloseIf(Some(2))).0, Some(1));
        assert_eq!(shown.clone().reduce(Show::CloseIf(Some(1))).0, None);
        assert_eq!(shown.reduce(Show::Close).0, None);
    }

    #[test]
    fn in_flight_tracks_several_keys() {
        let f = Rc::new(InFlight::default())
            .reduce(Flight::Start("a".into()))
            .reduce(Flight::Start("b".into()));
        assert!(f.has("a") && f.has("b"));
        let f = f.reduce(Flight::Done("b".into()));
        assert!(f.has("a") && !f.has("b"));
    }
}
