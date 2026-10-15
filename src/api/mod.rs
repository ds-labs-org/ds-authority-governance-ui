//! Typed access to the two backends this console drives: the hub's Issuer
//! Admin API (`v1alpha`, under `issuer_admin_api_path`) and the trusted
//! issuers registry (under `registry_api_path`). See
//! `governance/api-contract.md` for the wire contract.
//!
//! Everything except [`client`] is pure and unit-tested on the host:
//! [`requests`] turns a [`Config`](crate::config::Config) plus arguments into
//! an [`ApiRequest`] (method, same-origin URL, JSON body), [`error`] maps
//! failed responses to user-facing messages, and [`models`] holds the serde
//! types. [`client`] is the thin reqwest layer that executes a request.

pub mod client;
pub mod encoding;
pub mod error;
pub mod models;
pub mod query;
pub mod requests;

pub use error::ApiError;
pub use models::*;
pub use query::QuerySpec;
pub use requests::{Api, ApiRequest, IssuerFilter};
