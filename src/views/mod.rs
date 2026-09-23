//! Route views. Each is currently a one-line placeholder - filled in
//! separately, by other agents, once the shell (routing, config, store) is
//! in place.

mod credential_definitions;
mod credentials;
mod dashboard;
mod holders;
mod identity_bootstrap;

pub use credential_definitions::CredentialDefinitions;
pub use credentials::Credentials;
pub use dashboard::Dashboard;
pub use holders::Holders;
pub use identity_bootstrap::IdentityBootstrap;
