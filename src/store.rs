//! Application-wide state, shared via yewdux so it can be read from any
//! route view without re-fetching or re-deriving it.

use yewdux::prelude::*;

use crate::config::Config;

/// The single yewdux store for this app.
///
/// `config` is loaded once at startup (see `src/main.rs`) and then read via
/// `yewdux::use_store::<AppState>()` everywhere else - nothing downstream of
/// startup should fetch `configuration.json` itself.
#[derive(Clone, Default, PartialEq, Store)]
pub struct AppState {
    pub selected_participant_context: Option<String>,
    pub config: Option<Config>,
}
