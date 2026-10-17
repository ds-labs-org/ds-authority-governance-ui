//! Client-side form validation. The backends validate again; this only
//! catches obvious mistakes early, with a message next to the field.
//! Pure functions, unit-tested on the host.

use crate::api::{CredentialDefinition, IssuerRecord, NewHolder, RegistryStatus, Role};
use crate::format::{parse_date_input, split_list};

const DID_MESSAGE: &str = "Must be a DID, e.g. did:web:example.com.";

/// Per-field messages for the credential definition form.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DefinitionErrors {
    pub id: Option<String>,
    pub credential_type: Option<String>,
    pub validity: Option<String>,
}

/// Validates the credential definition form; on success returns the
/// definition to send (hub defaults for everything not on the form).
pub fn validate_definition(
    id: &str,
    credential_type: &str,
    validity: &str,
) -> Result<CredentialDefinition, DefinitionErrors> {
    let (id, credential_type, validity) = (id.trim(), credential_type.trim(), validity.trim());
    let mut errors = DefinitionErrors::default();
    if id.is_empty() {
        errors.id = Some("ID is required.".into());
    } else if id.chars().any(char::is_whitespace) {
        errors.id = Some("ID must not contain whitespace.".into());
    }
    if credential_type.is_empty() {
        errors.credential_type = Some("Credential type is required.".into());
    } else if !is_valid_credential_type(credential_type) {
        errors.credential_type = Some(
            "Must start with a letter and contain only letters, digits, '_', '.' or '-'.".into(),
        );
    }
    let parsed = validity.parse::<u64>().ok().filter(|v| *v > 0);
    if validity.is_empty() {
        errors.validity = Some("Validity is required.".into());
    } else if parsed.is_none() {
        errors.validity = Some("Validity must be a whole number of seconds greater than 0.".into());
    }
    match parsed {
        Some(validity) if errors == DefinitionErrors::default() => {
            Ok(CredentialDefinition::new(id, credential_type, validity))
        }
        _ => Err(errors),
    }
}

/// `^[A-Za-z][A-Za-z0-9_.-]*$`, as the hub enforces for `credentialType`.
pub fn is_valid_credential_type(value: &str) -> bool {
    let mut chars = value.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
}

/// `did:<method>:<method-specific-id>` with a lowercase alphanumeric method.
pub fn is_did(value: &str) -> bool {
    let mut parts = value.splitn(3, ':');
    let (Some("did"), Some(method), Some(id)) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    !method.is_empty()
        && method
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && !id.is_empty()
        && !id.chars().any(char::is_whitespace)
}

/// Whether `value` may name a registry party: a DID (DCP issuers) or an
/// https OID4VCI credential issuer URL. The registry applies the full
/// rules; this catches typos before a round trip.
pub fn is_issuer_id(value: &str) -> bool {
    is_did(value) || is_credential_issuer_url(value)
}

/// `https://` and a host, optionally a port and path; no query, fragment
/// or whitespace.
fn is_credential_issuer_url(value: &str) -> bool {
    value.strip_prefix("https://").is_some_and(|rest| {
        let host = rest.split(['/', ':']).next().unwrap_or_default();
        !host.is_empty()
            && !rest
                .chars()
                .any(|c| c.is_whitespace() || c == '?' || c == '#')
    })
}

const ISSUER_ID_MESSAGE: &str = "Must be a DID or an https credential issuer URL, e.g. did:web:example.com or https://issuer.example.";

fn issuer_id_error(value: &str, required: &str) -> Option<String> {
    if value.is_empty() {
        Some(required.to_string())
    } else if !is_issuer_id(value) {
        Some(ISSUER_ID_MESSAGE.to_string())
    } else {
        None
    }
}

/// The non-empty lines of `value`, each one base64 DER certificate.
fn split_anchors(value: &str) -> Result<Vec<String>, String> {
    let anchors: Vec<String> = value
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect();
    let base64 = |l: &String| {
        l.bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'+' | b'/' | b'='))
    };
    if anchors.iter().all(base64) {
        Ok(anchors)
    } else {
        Err("One base64 DER certificate per line (no PEM headers).".into())
    }
}

fn did_error(value: &str, required: &str) -> Option<String> {
    if value.is_empty() {
        Some(required.to_string())
    } else if !is_did(value) {
        Some(DID_MESSAGE.to_string())
    } else {
        None
    }
}

fn non_empty(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct HolderErrors {
    pub did: Option<String>,
}

pub fn validate_holder(did: &str, name: &str) -> Result<NewHolder, HolderErrors> {
    let did = did.trim();
    match did_error(did, "DID is required.") {
        Some(message) => Err(HolderErrors { did: Some(message) }),
        None => Ok(NewHolder {
            holder_id: None,
            did: did.to_string(),
            name: non_empty(name),
        }),
    }
}

/// Raw values of the trusted issuer form.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct IssuerForm {
    pub id: String,
    pub name: String,
    /// Wire value: `root_tao`, `tao` or `ti`.
    pub role: String,
    /// Comma-separated.
    pub credential_types: String,
    pub accredited_by: String,
    /// `YYYY-MM-DD` or empty.
    pub valid_from: String,
    /// `YYYY-MM-DD` or empty.
    pub valid_until: String,
    /// X.509 trust anchors, one base64 DER certificate per line.
    pub x509_certificates: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct IssuerErrors {
    pub id: Option<String>,
    pub role: Option<String>,
    pub credential_types: Option<String>,
    pub accredited_by: Option<String>,
    pub valid_from: Option<String>,
    pub valid_until: Option<String>,
    pub x509_certificates: Option<String>,
}

impl IssuerForm {
    /// Validates the form into the record to PUT, keeping `status`.
    // A few Option<String>s, built once per submit: boxing buys nothing.
    #[allow(clippy::result_large_err)]
    pub fn validate(&self, status: RegistryStatus) -> Result<IssuerRecord, IssuerErrors> {
        let id = self.id.trim();
        let accredited_by = self.accredited_by.trim();
        let role = Role::parse(self.role.trim());
        let types = split_list(&self.credential_types);
        let mut errors = IssuerErrors {
            id: issuer_id_error(id, "Issuer id is required."),
            ..Default::default()
        };

        if role.is_none() {
            errors.role = Some("Choose a role.".into());
        }
        if types.is_empty() {
            errors.credential_types = Some("At least one credential type is required.".into());
        } else if role.is_some_and(|r| r != Role::RootTao) && types.iter().any(|t| t == "*") {
            errors.credential_types = Some("Only a Root TAO may use the wildcard '*'.".into());
        }
        match role {
            Some(Role::RootTao) if !accredited_by.is_empty() => {
                errors.accredited_by =
                    Some("A Root TAO has no accreditor; leave this empty.".into());
            }
            Some(Role::Tao | Role::Ti) => {
                errors.accredited_by =
                    issuer_id_error(accredited_by, "Required for a TAO or Trusted issuer.");
            }
            _ => {}
        }

        let x509_certificates = split_anchors(&self.x509_certificates).unwrap_or_else(|e| {
            errors.x509_certificates = Some(e);
            Vec::new()
        });
        let valid_from = parse_date_input(&self.valid_from).unwrap_or_else(|e| {
            errors.valid_from = Some(e);
            None
        });
        let valid_until = parse_date_input(&self.valid_until).unwrap_or_else(|e| {
            errors.valid_until = Some(e);
            None
        });
        if let (Some(from), Some(until)) = (valid_from, valid_until)
            && until <= from
        {
            errors.valid_until = Some("Valid until must be after valid from.".into());
        }

        match role {
            Some(role) if errors == IssuerErrors::default() => Ok(IssuerRecord {
                id: id.to_string(),
                role,
                name: non_empty(&self.name),
                credential_types: types,
                accredited_by: non_empty(accredited_by),
                valid_from,
                valid_until,
                status,
                x509_certificates,
                updated_at: 0,
            }),
            _ => Err(errors),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::Role;

    #[test]
    fn credential_type_pattern() {
        for ok in ["MembershipCredential", "a", "A1_b.c-d"] {
            assert!(is_valid_credential_type(ok), "{ok}");
        }
        for bad in ["", "1Cred", "_x", "Has space", "Ümlaut", "a/b"] {
            assert!(!is_valid_credential_type(bad), "{bad}");
        }
    }

    #[test]
    fn did_shape() {
        for ok in [
            "did:web:example.com",
            "did:web:localhost%3A8081:issuer",
            "did:key:z6Mk",
        ] {
            assert!(is_did(ok), "{ok}");
        }
        for bad in [
            "",
            "did:",
            "did:web",
            "did:web:",
            "did:Web:x",
            "web:x",
            "did: web:x",
            "did:web:a b",
        ] {
            assert!(!is_did(bad), "{bad}");
        }
    }

    #[test]
    fn valid_definition_builds_hub_body() {
        let def = validate_definition(" membership ", " MembershipCredential ", " 7200 ").unwrap();
        assert_eq!(
            def,
            CredentialDefinition::new("membership", "MembershipCredential", 7200)
        );
    }

    #[test]
    fn definition_errors_are_per_field() {
        assert_eq!(
            validate_definition("", "", ""),
            Err(DefinitionErrors {
                id: Some("ID is required.".into()),
                credential_type: Some("Credential type is required.".into()),
                validity: Some("Validity is required.".into()),
            })
        );
        let errs = validate_definition("a b", "1x", "0").unwrap_err();
        assert_eq!(errs.id.as_deref(), Some("ID must not contain whitespace."));
        assert_eq!(
            errs.credential_type.as_deref(),
            Some("Must start with a letter and contain only letters, digits, '_', '.' or '-'.")
        );
        assert_eq!(
            errs.validity.as_deref(),
            Some("Validity must be a whole number of seconds greater than 0.")
        );
        assert!(
            validate_definition("a", "A", "1.5")
                .unwrap_err()
                .validity
                .is_some()
        );
        assert!(
            validate_definition("a", "A", "-3")
                .unwrap_err()
                .validity
                .is_some()
        );
    }

    #[test]
    fn holder_validation() {
        assert_eq!(
            validate_holder(" did:web:h ", "  "),
            Ok(NewHolder {
                did: "did:web:h".into(),
                ..Default::default()
            })
        );
        assert_eq!(
            validate_holder("did:web:h", " ACME "),
            Ok(NewHolder {
                did: "did:web:h".into(),
                name: Some("ACME".into()),
                ..Default::default()
            })
        );
        assert_eq!(
            validate_holder("", "x").unwrap_err().did.as_deref(),
            Some("DID is required.")
        );
        assert_eq!(
            validate_holder("holder", "x").unwrap_err().did.as_deref(),
            Some("Must be a DID, e.g. did:web:example.com.")
        );
    }

    fn form() -> IssuerForm {
        IssuerForm {
            id: "did:web:ti".into(),
            name: "Issuer".into(),
            role: "ti".into(),
            credential_types: "MembershipCredential, Other".into(),
            accredited_by: "did:web:root".into(),
            valid_from: "".into(),
            valid_until: "2027-01-01".into(),
            x509_certificates: "".into(),
        }
    }

    #[test]
    fn issuer_ids_are_dids_or_https_urls() {
        for ok in [
            "did:web:example.com",
            "https://issuer.example",
            "https://issuer.example:8443/tenants/acme",
        ] {
            assert!(is_issuer_id(ok), "{ok}");
        }
        for bad in [
            "http://issuer.example",
            "https://",
            "https://issuer.example/?x=1",
            "https://issuer.example/#f",
            "https://issuer example",
            "issuer.example",
        ] {
            assert!(!is_issuer_id(bad), "{bad}");
        }
    }

    #[test]
    fn an_oid4vci_issuer_with_anchors_validates() {
        let f = IssuerForm {
            id: " https://mdl.example ".into(),
            accredited_by: "https://tao.example/tenants/x".into(),
            credential_types: "org.iso.18013.5.1.mDL".into(),
            x509_certificates: " MIIBVzCB/qADAg== \n\n MIIBWDCB+qADAgECAhQ= ".into(),
            ..form()
        };
        let rec = f.validate(RegistryStatus::Active).unwrap();
        assert_eq!(rec.id, "https://mdl.example");
        assert_eq!(
            rec.accredited_by.as_deref(),
            Some("https://tao.example/tenants/x")
        );
        assert_eq!(
            rec.x509_certificates,
            vec![
                "MIIBVzCB/qADAg==".to_string(),
                "MIIBWDCB+qADAgECAhQ=".to_string()
            ]
        );

        let bad = IssuerForm {
            id: "http://mdl.example".into(),
            x509_certificates: "-----BEGIN CERTIFICATE-----".into(),
            ..form()
        };
        let e = bad.validate(RegistryStatus::Active).unwrap_err();
        assert_eq!(
            e.id.as_deref(),
            Some(
                "Must be a DID or an https credential issuer URL, e.g. did:web:example.com or https://issuer.example."
            )
        );
        assert_eq!(
            e.x509_certificates.as_deref(),
            Some("One base64 DER certificate per line (no PEM headers).")
        );
        let missing = IssuerForm {
            id: " ".into(),
            ..form()
        };
        assert_eq!(
            missing
                .validate(RegistryStatus::Active)
                .unwrap_err()
                .id
                .as_deref(),
            Some("Issuer id is required.")
        );
    }

    #[test]
    fn valid_issuer_form_builds_record() {
        let rec = form().validate(RegistryStatus::Suspended).unwrap();
        assert_eq!(
            rec,
            IssuerRecord {
                id: "did:web:ti".into(),
                role: Role::Ti,
                name: Some("Issuer".into()),
                credential_types: vec!["MembershipCredential".into(), "Other".into()],
                accredited_by: Some("did:web:root".into()),
                valid_from: None,
                valid_until: Some(1798761600),
                status: RegistryStatus::Suspended,
                x509_certificates: Vec::new(),
                updated_at: 0,
            }
        );
    }

    #[test]
    fn root_tao_needs_no_accreditor_and_may_use_wildcard() {
        let f = IssuerForm {
            role: "root_tao".into(),
            credential_types: "*".into(),
            accredited_by: "".into(),
            name: "".into(),
            ..form()
        };
        let rec = f.validate(RegistryStatus::Active).unwrap();
        assert_eq!(rec.accredited_by, None);
        assert_eq!(rec.name, None);
        assert_eq!(rec.credential_types, vec!["*"]);

        let with_accreditor = IssuerForm {
            role: "root_tao".into(),
            ..form()
        };
        assert_eq!(
            with_accreditor
                .validate(RegistryStatus::Active)
                .unwrap_err()
                .accredited_by
                .as_deref(),
            Some("A Root TAO has no accreditor; leave this empty.")
        );
    }

    #[test]
    fn issuer_form_errors() {
        let f = IssuerForm {
            id: "nope".into(),
            role: "".into(),
            credential_types: " , ".into(),
            accredited_by: "".into(),
            valid_from: "2027-02-01".into(),
            valid_until: "2027-01-01".into(),
            ..form()
        };
        let e = f.validate(RegistryStatus::Active).unwrap_err();
        assert_eq!(e.id.as_deref(), Some(ISSUER_ID_MESSAGE));
        assert_eq!(e.role.as_deref(), Some("Choose a role."));
        assert_eq!(
            e.credential_types.as_deref(),
            Some("At least one credential type is required.")
        );
        assert_eq!(
            e.valid_until.as_deref(),
            Some("Valid until must be after valid from.")
        );

        let ti_no_accreditor = IssuerForm {
            accredited_by: " ".into(),
            ..form()
        };
        assert_eq!(
            ti_no_accreditor
                .validate(RegistryStatus::Active)
                .unwrap_err()
                .accredited_by
                .as_deref(),
            Some("Required for a TAO or Trusted issuer.")
        );
        let bad_accreditor = IssuerForm {
            accredited_by: "root".into(),
            ..form()
        };
        assert_eq!(
            bad_accreditor
                .validate(RegistryStatus::Active)
                .unwrap_err()
                .accredited_by
                .as_deref(),
            Some(ISSUER_ID_MESSAGE)
        );
        let wildcard_ti = IssuerForm {
            credential_types: "*".into(),
            ..form()
        };
        assert_eq!(
            wildcard_ti
                .validate(RegistryStatus::Active)
                .unwrap_err()
                .credential_types
                .as_deref(),
            Some("Only a Root TAO may use the wildcard '*'.")
        );
        let bad_date = IssuerForm {
            valid_from: "2027-02-30".into(),
            ..form()
        };
        assert!(
            bad_date
                .validate(RegistryStatus::Active)
                .unwrap_err()
                .valid_from
                .is_some()
        );
    }
}
