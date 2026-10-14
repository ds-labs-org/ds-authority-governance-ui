//! Maps failed calls to messages a governance operator can act on.

use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub enum ApiError {
    /// The server answered with a non-success status.
    Http { status: u16, message: String },
    /// The request never got an HTTP answer (network, CORS, proxy down).
    Network(String),
    /// A success response whose body did not parse as expected.
    Decode(String),
}

impl ApiError {
    /// Builds the error for a non-success `status` with response `body`,
    /// preferring the server's `{"error": "..."}` message.
    pub fn from_response(status: u16, body: &str) -> Self {
        let _ = (status, body);
        todo!()
    }

    pub fn status(&self) -> Option<u16> {
        todo!()
    }
}

/// Extracts `error` from a `{"error": "..."}` body, if that's what it is.
pub fn server_message(body: &str) -> Option<String> {
    let _ = body;
    todo!()
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let _ = f;
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(status: u16, body: &str) -> String {
        ApiError::from_response(status, body).to_string()
    }

    #[test]
    fn server_message_reads_error_field() {
        assert_eq!(
            server_message(r#"{"error": "boom"}"#),
            Some("boom".to_string())
        );
        assert_eq!(server_message("not json"), None);
        assert_eq!(server_message(r#"{"message": "x"}"#), None);
        assert_eq!(server_message(r#"{"error": "  "}"#), None);
    }

    #[test]
    fn bad_request_includes_server_message() {
        assert_eq!(
            msg(400, r#"{"error": "missing field `did`"}"#),
            "Invalid request: missing field `did`"
        );
    }

    #[test]
    fn unauthorized_points_at_the_gateway_token() {
        assert_eq!(
            msg(401, r#"{"error": "missing or wrong admin token"}"#),
            "Not authorized: missing or wrong admin token. Check the gateway's admin token."
        );
    }

    #[test]
    fn forbidden_and_not_found_and_conflict() {
        assert_eq!(
            msg(403, r#"{"error": "admin API disabled"}"#),
            "Forbidden: admin API disabled"
        );
        assert_eq!(
            msg(404, r#"{"error": "no such holder"}"#),
            "Not found: no such holder"
        );
        assert_eq!(
            msg(409, r#"{"error": "did:web:a still accredits did:web:b"}"#),
            "Conflict: did:web:a still accredits did:web:b"
        );
    }

    #[test]
    fn unprocessable_entity_is_a_rule_violation() {
        assert_eq!(
            msg(422, r#"{"error": "scope wider than accreditor"}"#),
            "Rejected: scope wider than accreditor"
        );
    }

    #[test]
    fn server_errors_mention_status() {
        assert_eq!(
            msg(503, r#"{"error": "holder capacity reached"}"#),
            "Server error (503): holder capacity reached"
        );
        assert_eq!(msg(500, ""), "Server error (500): Internal Server Error");
    }

    #[test]
    fn falls_back_to_short_plain_body_then_reason_phrase() {
        assert_eq!(msg(404, "nope"), "Not found: nope");
        assert_eq!(msg(404, ""), "Not found: Not Found");
        assert_eq!(
            msg(401, ""),
            "Not authorized: Unauthorized. Check the gateway's admin token."
        );
        let long = "x".repeat(500);
        assert_eq!(msg(409, &long), "Conflict: Conflict");
        assert_eq!(
            msg(502, "<html>bad gateway</html>"),
            "Server error (502): Bad Gateway"
        );
    }

    #[test]
    fn other_statuses_are_unexpected() {
        assert_eq!(msg(418, ""), "Unexpected response (418)");
        assert_eq!(
            msg(418, r#"{"error":"teapot"}"#),
            "Unexpected response (418): teapot"
        );
    }

    #[test]
    fn status_accessor_and_non_http_display() {
        assert_eq!(ApiError::from_response(409, "").status(), Some(409));
        assert_eq!(ApiError::Network("x".into()).status(), None);
        assert_eq!(
            ApiError::Network("failed to fetch".into()).to_string(),
            "Could not reach the server: failed to fetch"
        );
        assert_eq!(
            ApiError::Decode("expected array".into()).to_string(),
            "Unexpected response from the server: expected array"
        );
    }
}
