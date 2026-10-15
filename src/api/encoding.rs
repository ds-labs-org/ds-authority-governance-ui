//! Path-segment encodings used by both APIs.

/// Base64 of a participant context id, for `/v1alpha/participants/{pc}`
/// (EDC base64-encodes the id there). The hub accepts both
/// alphabets, padded or not; we send url-safe without padding so the
/// segment never contains `/`, `+` or `=`.
pub fn base64_url(input: &str) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        let symbols = chunk.len() + 1;
        for i in 0..symbols {
            out.push(ALPHABET[((n >> (18 - 6 * i)) & 0x3f) as usize] as char);
        }
    }
    out
}

/// Percent-encodes `input` as one path segment or query value, exactly like
/// JavaScript's `encodeURIComponent`: everything except
/// `A-Z a-z 0-9 - _ . ! ~ * ' ( )` is UTF-8 percent-encoded. A DID such as
/// `did:web:h%3A1` therefore travels as `did%3Aweb%3Ah%253A1`.
pub fn encode_component(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&byte) {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_url_encodes_default_participant_context() {
        assert_eq!(base64_url("issuer"), "aXNzdWVy");
    }

    #[test]
    fn base64_url_omits_padding_and_uses_url_safe_alphabet() {
        assert_eq!(base64_url("a"), "YQ");
        assert_eq!(base64_url("ab"), "YWI");
        assert_eq!(base64_url("abc"), "YWJj");
        // "Pz8+" / "Pz8/" in the standard alphabet.
        assert_eq!(base64_url("??>"), "Pz8-");
        assert_eq!(base64_url("???"), "Pz8_");
        assert_eq!(base64_url(""), "");
    }

    #[test]
    fn encode_component_keeps_unreserved_characters() {
        assert_eq!(encode_component("AZaz09-_.!~*'()"), "AZaz09-_.!~*'()");
    }

    #[test]
    fn encode_component_encodes_did_including_existing_percent() {
        assert_eq!(encode_component("did:web:h%3A1"), "did%3Aweb%3Ah%253A1");
        assert_eq!(
            encode_component("did:web:localhost%3A8081:issuer-service"),
            "did%3Aweb%3Alocalhost%253A8081%3Aissuer-service"
        );
    }

    #[test]
    fn encode_component_encodes_reserved_and_non_ascii() {
        assert_eq!(
            encode_component("a b/c?d#e&f=g+h"),
            "a%20b%2Fc%3Fd%23e%26f%3Dg%2Bh"
        );
        assert_eq!(encode_component("é"), "%C3%A9");
        assert_eq!(encode_component("urn:uuid:1"), "urn%3Auuid%3A1");
    }
}
