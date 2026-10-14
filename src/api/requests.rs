//! Request builders: every endpoint the console calls, as a pure function
//! from [`Config`] + arguments to an [`ApiRequest`].
//!
//! Hub paths live under
//! `{issuer_admin_api_path}/v1alpha/participants/{base64(participant_context_id)}`;
//! registry paths under `{registry_api_path}/v1`. Ids and DIDs are always
//! sent as one fully percent-encoded segment.

use serde::Serialize;
use serde_json::Value;

use super::encoding::{base64_url, encode_component};
use super::models::{CredentialDefinition, IssuerRecord, NewHolder, RegistryStatus, Role};
use super::query::QuerySpec;
use crate::config::Config;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

/// A request ready to execute: same-origin relative URL plus optional JSON
/// body.
#[derive(Clone, Debug, PartialEq)]
pub struct ApiRequest {
    pub method: Method,
    pub url: String,
    pub body: Option<Value>,
}

/// Filters for `GET /v1/issuers`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct IssuerFilter {
    pub role: Option<Role>,
    pub status: Option<RegistryStatus>,
    pub credential_type: Option<String>,
}

/// Builds requests from the runtime configuration.
#[derive(Clone, Copy, Debug)]
pub struct Api<'a> {
    pub config: &'a Config,
}

impl<'a> Api<'a> {
    pub fn new(config: &'a Config) -> Self {
        Self { config }
    }

    // ------------------------------------------------------------ hub ---

    /// `{issuer_admin_api_path}/v1alpha/participants/{pc}`.
    pub fn participant_base(&self) -> String {
        todo!()
    }

    pub fn participant(&self) -> ApiRequest {
        todo!()
    }

    pub fn query_credential_definitions(&self, query: &QuerySpec) -> ApiRequest {
        let _ = query;
        todo!()
    }

    pub fn create_credential_definition(&self, definition: &CredentialDefinition) -> ApiRequest {
        let _ = definition;
        todo!()
    }

    pub fn update_credential_definition(&self, definition: &CredentialDefinition) -> ApiRequest {
        let _ = definition;
        todo!()
    }

    pub fn delete_credential_definition(&self, id: &str) -> ApiRequest {
        let _ = id;
        todo!()
    }

    pub fn query_holders(&self, query: &QuerySpec) -> ApiRequest {
        let _ = query;
        todo!()
    }

    pub fn register_holder(&self, holder: &NewHolder) -> ApiRequest {
        let _ = holder;
        todo!()
    }

    pub fn approve_holder(&self, holder_id: &str) -> ApiRequest {
        let _ = holder_id;
        todo!()
    }

    pub fn reject_holder(&self, holder_id: &str) -> ApiRequest {
        let _ = holder_id;
        todo!()
    }

    pub fn query_issuance_processes(&self, query: &QuerySpec) -> ApiRequest {
        let _ = query;
        todo!()
    }

    pub fn query_credentials(&self, query: &QuerySpec) -> ApiRequest {
        let _ = query;
        todo!()
    }

    pub fn revoke_credential(&self, id: &str) -> ApiRequest {
        let _ = id;
        todo!()
    }

    // ------------------------------------------------------- registry ---

    pub fn list_issuers(&self, filter: &IssuerFilter) -> ApiRequest {
        let _ = filter;
        todo!()
    }

    pub fn get_issuer(&self, did: &str) -> ApiRequest {
        let _ = did;
        todo!()
    }

    pub fn put_issuer(&self, record: &IssuerRecord) -> ApiRequest {
        let _ = record;
        todo!()
    }

    pub fn set_issuer_status(&self, did: &str, status: RegistryStatus) -> ApiRequest {
        let _ = (did, status);
        todo!()
    }

    pub fn delete_issuer(&self, did: &str) -> ApiRequest {
        let _ = did;
        todo!()
    }

    pub fn check_trust(&self, did: &str, credential_type: &str) -> ApiRequest {
        let _ = (did, credential_type);
        todo!()
    }

    /// `GET /v1/admin`: 204 when the console may write to the registry.
    pub fn registry_admin_probe(&self) -> ApiRequest {
        todo!()
    }
}

#[allow(dead_code)]
fn json_body(value: &impl Serialize) -> Option<Value> {
    Some(serde_json::to_value(value).expect("API models always serialize"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::query::SortOrder;
    use serde_json::json;

    fn config() -> Config {
        Config {
            identity_api_path: "/api/identity".into(),
            issuer_admin_api_path: "/api/issuer".into(),
            registry_api_path: "/api/registry".into(),
            participant_context_id: "issuer".into(),
            bearer_token: None,
        }
    }

    const PC: &str = "/api/issuer/v1alpha/participants/aXNzdWVy";

    fn req(method: Method, url: &str, body: Option<Value>) -> ApiRequest {
        ApiRequest {
            method,
            url: url.to_string(),
            body,
        }
    }

    #[test]
    fn participant_base_encodes_context_id() {
        let c = config();
        assert_eq!(Api::new(&c).participant_base(), PC);
        assert_eq!(Api::new(&c).participant(), req(Method::Get, PC, None));
    }

    #[test]
    fn base_paths_tolerate_trailing_slash() {
        let mut c = config();
        c.issuer_admin_api_path = "/api/issuer/".into();
        c.registry_api_path = "/api/registry/".into();
        c.participant_context_id = "other".into();
        let api = Api::new(&c);
        assert_eq!(
            api.participant_base(),
            "/api/issuer/v1alpha/participants/b3RoZXI"
        );
        assert_eq!(api.registry_admin_probe().url, "/api/registry/v1/admin");
    }

    #[test]
    fn credential_definition_requests() {
        let c = config();
        let api = Api::new(&c);
        let q = QuerySpec::new().sort("createdAt", SortOrder::Desc);
        assert_eq!(
            api.query_credential_definitions(&q),
            req(
                Method::Post,
                &format!("{PC}/credentialdefinitions/query"),
                Some(json!({"sortField": "createdAt", "sortOrder": "DESC"}))
            )
        );
        let def = CredentialDefinition::new("membership-credential", "MembershipCredential", 3600);
        let body = Some(serde_json::to_value(&def).unwrap());
        assert_eq!(
            api.create_credential_definition(&def),
            req(
                Method::Post,
                &format!("{PC}/credentialdefinitions"),
                body.clone()
            )
        );
        assert_eq!(
            api.update_credential_definition(&def),
            req(Method::Put, &format!("{PC}/credentialdefinitions"), body)
        );
        assert_eq!(
            api.delete_credential_definition("a b/c"),
            req(
                Method::Delete,
                &format!("{PC}/credentialdefinitions/a%20b%2Fc"),
                None
            )
        );
    }

    #[test]
    fn holder_requests() {
        let c = config();
        let api = Api::new(&c);
        let q = QuerySpec::new().filter("status", "pending");
        assert_eq!(
            api.query_holders(&q),
            req(
                Method::Post,
                &format!("{PC}/holders/query"),
                Some(
                    json!({"filterExpression": [{"operandLeft": "status", "operator": "=", "operandRight": "pending"}]})
                )
            )
        );
        let new = NewHolder {
            did: "did:web:h".into(),
            name: Some("ACME".into()),
            ..Default::default()
        };
        assert_eq!(
            api.register_holder(&new),
            req(
                Method::Post,
                &format!("{PC}/holders"),
                Some(json!({"did": "did:web:h", "name": "ACME"}))
            )
        );
        assert_eq!(
            api.approve_holder("c0ffee"),
            req(Method::Post, &format!("{PC}/holders/c0ffee/approve"), None)
        );
        assert_eq!(
            api.reject_holder("c0 ffee"),
            req(
                Method::Post,
                &format!("{PC}/holders/c0%20ffee/reject"),
                None
            )
        );
    }

    #[test]
    fn issuance_and_credential_requests() {
        let c = config();
        let api = Api::new(&c);
        assert_eq!(
            api.query_issuance_processes(&QuerySpec::new()),
            req(
                Method::Post,
                &format!("{PC}/issuanceprocesses/query"),
                Some(json!({}))
            )
        );
        assert_eq!(
            api.query_credentials(&QuerySpec::new()),
            req(
                Method::Post,
                &format!("{PC}/credentials/query"),
                Some(json!({}))
            )
        );
        assert_eq!(
            api.revoke_credential("urn:uuid:1"),
            req(
                Method::Post,
                &format!("{PC}/credentials/urn%3Auuid%3A1/revoke"),
                None
            )
        );
    }

    #[test]
    fn list_issuers_builds_query_string() {
        let c = config();
        let api = Api::new(&c);
        assert_eq!(
            api.list_issuers(&IssuerFilter::default()),
            req(Method::Get, "/api/registry/v1/issuers", None)
        );
        let filter = IssuerFilter {
            role: Some(Role::RootTao),
            status: Some(RegistryStatus::Suspended),
            credential_type: Some("Membership Credential".into()),
        };
        assert_eq!(
            api.list_issuers(&filter).url,
            "/api/registry/v1/issuers?role=root_tao&status=suspended&credentialType=Membership%20Credential"
        );
        let only_status = IssuerFilter {
            status: Some(RegistryStatus::Active),
            ..Default::default()
        };
        assert_eq!(
            api.list_issuers(&only_status).url,
            "/api/registry/v1/issuers?status=active"
        );
    }

    #[test]
    fn registry_record_requests_encode_did_as_one_segment() {
        let c = config();
        let api = Api::new(&c);
        let did = "did:web:h%3A1";
        let seg = "/api/registry/v1/issuers/did%3Aweb%3Ah%253A1";
        assert_eq!(api.get_issuer(did), req(Method::Get, seg, None));
        assert_eq!(api.delete_issuer(did), req(Method::Delete, seg, None));
        assert_eq!(
            api.set_issuer_status(did, RegistryStatus::Suspended),
            req(
                Method::Patch,
                &format!("{seg}/status"),
                Some(json!({"status": "suspended"}))
            )
        );
        assert_eq!(
            api.check_trust(did, "MembershipCredential"),
            req(
                Method::Get,
                &format!("{seg}/types/MembershipCredential"),
                None
            )
        );
        let record = IssuerRecord {
            did: did.into(),
            role: Role::Ti,
            name: Some("H".into()),
            credential_types: vec!["MembershipCredential".into()],
            accredited_by: Some("did:web:root".into()),
            valid_from: None,
            valid_until: Some(10),
            status: RegistryStatus::Active,
            updated_at: 99,
        };
        assert_eq!(
            api.put_issuer(&record),
            req(
                Method::Put,
                seg,
                Some(json!({
                    "did": did, "role": "ti", "name": "H",
                    "credentialTypes": ["MembershipCredential"],
                    "accreditedBy": "did:web:root", "validUntil": 10, "status": "active"
                }))
            )
        );
        assert_eq!(
            api.registry_admin_probe(),
            req(Method::Get, "/api/registry/v1/admin", None)
        );
    }
}
