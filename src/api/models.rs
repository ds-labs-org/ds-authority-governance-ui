//! Wire types for the hub Issuer Admin API and the trusted issuers
//! registry. Field names are camelCase on the wire; timestamps are unix
//! seconds. Enums carry an `Unknown` catch-all so a newer backend's value
//! never breaks a whole table.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// The only credential format the hub issues today.
pub const FORMAT_VC1_0_JWT: &str = "VC1_0_JWT";

/// Default credential validity (seconds), as the hub applies it.
pub const DEFAULT_VALIDITY: u64 = 3600;

// ---------------------------------------------------------------- hub ---

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Participant {
    pub participant_context_id: String,
    pub did: String,
    #[serde(default)]
    pub state: String,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialDefinition {
    pub id: String,
    pub credential_type: String,
    #[serde(default = "default_format")]
    pub format: String,
    #[serde(default = "default_validity")]
    pub validity: u64,
    #[serde(default)]
    pub json_schema_url: Option<String>,
    #[serde(default)]
    pub json_schema: Option<Value>,
    #[serde(default)]
    pub mappings: Vec<Value>,
    #[serde(default)]
    pub rules: Vec<Value>,
    #[serde(default)]
    pub attestations: Vec<Value>,
    /// Set by the hub; omitted from create/update bodies when unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<u64>,
}

fn default_format() -> String {
    FORMAT_VC1_0_JWT.to_string()
}

fn default_validity() -> u64 {
    DEFAULT_VALIDITY
}

impl CredentialDefinition {
    /// A fresh definition with the hub's defaults for everything but the
    /// two required fields.
    pub fn new(id: &str, credential_type: &str, validity: u64) -> Self {
        Self {
            id: id.to_string(),
            credential_type: credential_type.to_string(),
            format: default_format(),
            validity,
            json_schema_url: None,
            json_schema: None,
            mappings: Vec::new(),
            rules: Vec::new(),
            attestations: Vec::new(),
            created_at: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HolderStatus {
    Pending,
    Approved,
    Rejected,
    #[serde(other)]
    Unknown,
}

impl HolderStatus {
    pub const ALL: [HolderStatus; 3] = [
        HolderStatus::Pending,
        HolderStatus::Approved,
        HolderStatus::Rejected,
    ];

    /// Wire value, also used as the QuerySpec filter value.
    pub fn as_str(self) -> &'static str {
        match self {
            HolderStatus::Pending => "pending",
            HolderStatus::Approved => "approved",
            HolderStatus::Rejected => "rejected",
            HolderStatus::Unknown => "unknown",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            HolderStatus::Pending => "Pending",
            HolderStatus::Approved => "Approved",
            HolderStatus::Rejected => "Rejected",
            HolderStatus::Unknown => "Unknown",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Holder {
    pub holder_id: String,
    pub did: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub properties: Map<String, Value>,
    pub status: HolderStatus,
    #[serde(default)]
    pub created_at: u64,
}

/// Body of `POST .../holders` (admin registration).
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewHolder {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub holder_id: Option<String>,
    pub did: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProcessState {
    Submitted,
    Approved,
    Delivered,
    Errored,
    Rejected,
    #[serde(other)]
    Unknown,
}

impl ProcessState {
    pub fn as_str(self) -> &'static str {
        match self {
            ProcessState::Submitted => "SUBMITTED",
            ProcessState::Approved => "APPROVED",
            ProcessState::Delivered => "DELIVERED",
            ProcessState::Errored => "ERRORED",
            ProcessState::Rejected => "REJECTED",
            ProcessState::Unknown => "UNKNOWN",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IssuanceProcess {
    pub id: String,
    pub state: ProcessState,
    #[serde(default)]
    pub holder_id: String,
    #[serde(default)]
    pub holder_did: String,
    #[serde(default)]
    pub holder_pid: Option<String>,
    #[serde(default)]
    pub credential_definitions: Vec<String>,
    #[serde(default)]
    pub error_detail: Option<String>,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub state_timestamp: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CredentialStatus {
    Active,
    Revoked,
    #[serde(other)]
    Unknown,
}

impl CredentialStatus {
    pub fn label(self) -> &'static str {
        match self {
            CredentialStatus::Active => "Active",
            CredentialStatus::Revoked => "Revoked",
            CredentialStatus::Unknown => "Unknown",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IssuedCredential {
    pub id: String,
    pub credential_type: String,
    #[serde(default)]
    pub credential_definition_id: String,
    #[serde(default)]
    pub holder_id: String,
    #[serde(default)]
    pub holder_did: String,
    #[serde(default)]
    pub issuance_process_id: String,
    #[serde(default)]
    pub format: String,
    #[serde(default)]
    pub issued_at: u64,
    #[serde(default)]
    pub expires_at: Option<u64>,
    pub status: CredentialStatus,
    #[serde(default)]
    pub status_list_index: Option<u64>,
}

// ----------------------------------------------------------- registry ---

/// Tier in the registry's accreditation chain.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    RootTao,
    Tao,
    Ti,
}

impl Role {
    pub const ALL: [Role; 3] = [Role::RootTao, Role::Tao, Role::Ti];

    pub fn as_str(self) -> &'static str {
        match self {
            Role::RootTao => "root_tao",
            Role::Tao => "tao",
            Role::Ti => "ti",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Role::RootTao => "Root TAO",
            Role::Tao => "TAO",
            Role::Ti => "Trusted issuer",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|r| r.as_str() == value)
    }

    /// Root TAOs and TAOs may accredit others.
    pub fn can_accredit(self) -> bool {
        matches!(self, Role::RootTao | Role::Tao)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RegistryStatus {
    Active,
    Suspended,
    Revoked,
}

impl RegistryStatus {
    pub const ALL: [RegistryStatus; 3] = [
        RegistryStatus::Active,
        RegistryStatus::Suspended,
        RegistryStatus::Revoked,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            RegistryStatus::Active => "active",
            RegistryStatus::Suspended => "suspended",
            RegistryStatus::Revoked => "revoked",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            RegistryStatus::Active => "Active",
            RegistryStatus::Suspended => "Suspended",
            RegistryStatus::Revoked => "Revoked",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.as_str() == value)
    }
}

/// One registry record (`GET/PUT /v1/issuers/{did}`).
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IssuerRecord {
    pub did: String,
    pub role: Role,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default)]
    pub credential_types: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accredited_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_from: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_until: Option<u64>,
    pub status: RegistryStatus,
    /// Set by the registry; omitted from PUT bodies.
    #[serde(default, skip_serializing)]
    pub updated_at: u64,
}

/// `GET /v1/issuers` response.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct IssuerList {
    #[serde(default)]
    pub version: u64,
    #[serde(default)]
    pub total: u64,
    #[serde(default)]
    pub items: Vec<IssuerRecord>,
}

/// `GET /v1/issuers/{did}/types/{type}` response.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct TrustDecision {
    pub trusted: bool,
    #[serde(default)]
    pub chain: Vec<String>,
    #[serde(default)]
    pub reason: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn participant_parses_contract_example() {
        let p: Participant = serde_json::from_value(json!({
            "participantContextId": "issuer",
            "did": "did:web:localhost%3A8081:issuer-service",
            "state": "ACTIVATED"
        }))
        .unwrap();
        assert_eq!(p.did, "did:web:localhost%3A8081:issuer-service");
        assert_eq!(p.participant_context_id, "issuer");
    }

    #[test]
    fn credential_definition_round_trips_contract_example() {
        let wire = json!({
            "id": "membership-credential",
            "credentialType": "MembershipCredential",
            "format": "VC1_0_JWT",
            "validity": 3600,
            "jsonSchemaUrl": null,
            "jsonSchema": null,
            "mappings": [],
            "rules": [],
            "attestations": [],
            "createdAt": 1792017886
        });
        let def: CredentialDefinition = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(def.created_at, Some(1792017886));
        assert_eq!(serde_json::to_value(&def).unwrap(), wire);
    }

    #[test]
    fn new_credential_definition_body_omits_created_at() {
        let body = serde_json::to_value(CredentialDefinition::new("d", "T", 60)).unwrap();
        assert_eq!(
            body,
            json!({
                "id": "d", "credentialType": "T", "format": "VC1_0_JWT", "validity": 60,
                "jsonSchemaUrl": null, "jsonSchema": null,
                "mappings": [], "rules": [], "attestations": []
            })
        );
    }

    #[test]
    fn holder_parses_and_status_helpers() {
        let h: Holder = serde_json::from_value(json!({
            "holderId": "c0ffee", "did": "did:web:holder.example", "name": "ACME",
            "properties": {}, "status": "pending", "createdAt": 1792017886
        }))
        .unwrap();
        assert_eq!(h.status, HolderStatus::Pending);
        assert_eq!(HolderStatus::Pending.as_str(), "pending");
        assert_eq!(HolderStatus::Approved.as_str(), "approved");
        assert_eq!(HolderStatus::Rejected.as_str(), "rejected");
        assert_eq!(HolderStatus::Pending.label(), "Pending");
        assert_eq!(HolderStatus::Approved.label(), "Approved");
        assert_eq!(HolderStatus::Rejected.label(), "Rejected");
        let odd: HolderStatus = serde_json::from_value(json!("banned")).unwrap();
        assert_eq!(odd, HolderStatus::Unknown);
        assert_eq!(odd.label(), "Unknown");
    }

    #[test]
    fn new_holder_body_omits_unset_fields() {
        let body = NewHolder {
            did: "did:web:x".into(),
            ..Default::default()
        };
        assert_eq!(
            serde_json::to_value(body).unwrap(),
            json!({"did": "did:web:x"})
        );
    }

    #[test]
    fn issuance_process_parses() {
        let p: IssuanceProcess = serde_json::from_value(json!({
            "id": "u", "state": "SUBMITTED", "holderId": "h", "holderDid": "did:web:h",
            "holderPid": "pid", "credentialDefinitions": ["membership-credential"],
            "errorDetail": null, "createdAt": 1, "stateTimestamp": 2
        }))
        .unwrap();
        assert_eq!(p.state, ProcessState::Submitted);
        assert_eq!(ProcessState::Delivered.as_str(), "DELIVERED");
        assert_eq!(ProcessState::Unknown.as_str(), "UNKNOWN");
    }

    #[test]
    fn issued_credential_parses() {
        let c: IssuedCredential = serde_json::from_value(json!({
            "id": "urn:uuid:1", "credentialType": "MembershipCredential",
            "credentialDefinitionId": "membership-credential", "holderId": "h",
            "holderDid": "did:web:h", "issuanceProcessId": "p", "format": "VC1_0_JWT",
            "issuedAt": 1792017886, "expiresAt": 1792021486, "status": "revoked",
            "statusListIndex": 7
        }))
        .unwrap();
        assert_eq!(c.status, CredentialStatus::Revoked);
        assert_eq!(c.status_list_index, Some(7));
        assert_eq!(CredentialStatus::Active.label(), "Active");
        assert_eq!(CredentialStatus::Revoked.label(), "Revoked");
    }

    #[test]
    fn issuer_record_round_trip_omits_updated_at_and_unset() {
        let rec: IssuerRecord = serde_json::from_value(json!({
            "did": "did:web:ti", "role": "ti", "credentialTypes": ["MembershipCredential"],
            "accreditedBy": "did:web:root", "status": "active", "updatedAt": 5
        }))
        .unwrap();
        assert_eq!(rec.role, Role::Ti);
        assert_eq!(rec.updated_at, 5);
        assert_eq!(
            serde_json::to_value(&rec).unwrap(),
            json!({
                "did": "did:web:ti", "role": "ti", "credentialTypes": ["MembershipCredential"],
                "accreditedBy": "did:web:root", "status": "active"
            })
        );
    }

    #[test]
    fn issuer_records_are_read_by_id_and_written_with_id() {
        // The registry names a party `id` (a DID for DCP issuers, an https
        // URL for OID4VCI ones) and may list X.509 trust anchors; records
        // from before the rename still say `did`.
        let current = json!({
            "id": "https://issuer.example/tenants/acme", "role": "ti",
            "credentialTypes": ["https://credentials.example/pid"],
            "accreditedBy": "did:web:root", "status": "active",
            "x509Certificates": ["MIIB"], "updatedAt": 5
        });
        let rec: IssuerRecord = serde_json::from_value(current).unwrap();
        let written = serde_json::to_value(&rec).unwrap();
        assert_eq!(written["id"], "https://issuer.example/tenants/acme");
        assert!(written.get("did").is_none(), "{written}");
        assert_eq!(written["x509Certificates"], json!(["MIIB"]));

        let legacy: IssuerRecord = serde_json::from_value(json!({
            "did": "did:web:ti", "role": "ti", "credentialTypes": ["T"], "status": "active"
        }))
        .unwrap();
        let written = serde_json::to_value(&legacy).unwrap();
        assert_eq!(written["id"], "did:web:ti");
        assert!(written.get("x509Certificates").is_none(), "{written}");
    }

    #[test]
    fn role_and_status_helpers() {
        assert_eq!(Role::RootTao.as_str(), "root_tao");
        assert_eq!(Role::Tao.as_str(), "tao");
        assert_eq!(Role::Ti.as_str(), "ti");
        assert_eq!(Role::RootTao.label(), "Root TAO");
        assert_eq!(Role::Tao.label(), "TAO");
        assert_eq!(Role::Ti.label(), "Trusted issuer");
        assert_eq!(Role::parse("tao"), Some(Role::Tao));
        assert_eq!(Role::parse("x"), None);
        assert!(Role::RootTao.can_accredit());
        assert!(Role::Tao.can_accredit());
        assert!(!Role::Ti.can_accredit());
        assert_eq!(RegistryStatus::Suspended.as_str(), "suspended");
        assert_eq!(RegistryStatus::Revoked.label(), "Revoked");
        assert_eq!(
            RegistryStatus::parse("active"),
            Some(RegistryStatus::Active)
        );
        assert_eq!(RegistryStatus::parse(""), None);
    }

    #[test]
    fn issuer_list_and_trust_decision_parse() {
        let list: IssuerList = serde_json::from_value(json!({
            "version": 3, "total": 1,
            "items": [{"did": "did:web:root", "role": "root_tao", "credentialTypes": ["*"], "status": "active", "updatedAt": 1}]
        }))
        .unwrap();
        assert_eq!(list.items[0].role, Role::RootTao);
        let d: TrustDecision = serde_json::from_value(json!({
            "trusted": false, "chain": ["did:web:ti"], "reason": "suspended"
        }))
        .unwrap();
        assert!(!d.trusted);
        assert_eq!(d.reason.as_deref(), Some("suspended"));
        let ok: TrustDecision =
            serde_json::from_value(json!({"trusted": true, "chain": ["a", "b"]})).unwrap();
        assert_eq!(ok.reason, None);
    }
}
