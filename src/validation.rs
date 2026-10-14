//! Client-side form validation. The backends validate again; this only
//! catches obvious mistakes early, with a message next to the field.
//! Pure functions, unit-tested on the host.

use crate::api::{CredentialDefinition, IssuerRecord, NewHolder, RegistryStatus};

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
    let _ = (id, credential_type, validity);
    todo!()
}

/// `^[A-Za-z][A-Za-z0-9_.-]*$`, as the hub enforces for `credentialType`.
pub fn is_valid_credential_type(value: &str) -> bool {
    let _ = value;
    todo!()
}

/// `did:<method>:<method-specific-id>` with a lowercase alphanumeric method.
pub fn is_did(value: &str) -> bool {
    let _ = value;
    todo!()
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct HolderErrors {
    pub did: Option<String>,
}

pub fn validate_holder(did: &str, name: &str) -> Result<NewHolder, HolderErrors> {
    let _ = (did, name);
    todo!()
}

/// Raw values of the trusted issuer form.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct IssuerForm {
    pub did: String,
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
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct IssuerErrors {
    pub did: Option<String>,
    pub role: Option<String>,
    pub credential_types: Option<String>,
    pub accredited_by: Option<String>,
    pub valid_from: Option<String>,
    pub valid_until: Option<String>,
}

impl IssuerForm {
    /// Validates the form into the record to PUT, keeping `status`.
    pub fn validate(&self, status: RegistryStatus) -> Result<IssuerRecord, IssuerErrors> {
        let _ = status;
        todo!()
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
            did: "did:web:ti".into(),
            name: "Issuer".into(),
            role: "ti".into(),
            credential_types: "MembershipCredential, Other".into(),
            accredited_by: "did:web:root".into(),
            valid_from: "".into(),
            valid_until: "2027-01-01".into(),
        }
    }

    #[test]
    fn valid_issuer_form_builds_record() {
        let rec = form().validate(RegistryStatus::Suspended).unwrap();
        assert_eq!(
            rec,
            IssuerRecord {
                did: "did:web:ti".into(),
                role: Role::Ti,
                name: Some("Issuer".into()),
                credential_types: vec!["MembershipCredential".into(), "Other".into()],
                accredited_by: Some("did:web:root".into()),
                valid_from: None,
                valid_until: Some(1798761600),
                status: RegistryStatus::Suspended,
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
            did: "nope".into(),
            role: "".into(),
            credential_types: " , ".into(),
            accredited_by: "".into(),
            valid_from: "2027-02-01".into(),
            valid_until: "2027-01-01".into(),
            ..form()
        };
        let e = f.validate(RegistryStatus::Active).unwrap_err();
        assert_eq!(
            e.did.as_deref(),
            Some("Must be a DID, e.g. did:web:example.com.")
        );
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
            Some("Must be a DID, e.g. did:web:example.com.")
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
