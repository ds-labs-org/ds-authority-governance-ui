//! Route views. Each owns its data loading (see `crate::hooks::use_load`)
//! and talks to the backends through `crate::api`.

mod credential_definitions;
mod credentials;
mod dashboard;
mod holders;
mod identity_bootstrap;
mod trusted_issuers;

pub use credential_definitions::CredentialDefinitions;
pub use credentials::Credentials;
pub use dashboard::Dashboard;
pub use holders::Holders;
pub use identity_bootstrap::IdentityBootstrap;
pub use trusted_issuers::TrustedIssuers;
