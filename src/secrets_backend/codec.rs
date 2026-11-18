//! Value codec for backends that cannot store arbitrary strings verbatim.
//!
//! Infisical trims secret values server-side, so multi-line values (PEM/GPG
//! keys), values with surrounding whitespace and binary payloads are stored as
//! `diegops:b64:<base64>`. Plain single-line values are stored verbatim so they
//! stay readable in the Infisical UI.

/// Prefix marking a base64-encoded value.
pub const MARKER: &str = "diegops:b64:";

/// Encodes `value` for storage in Infisical.
pub fn encode(value: &str) -> String {
    if needs_encoding(value) {
        use base64::Engine as _;
        format!(
            "{MARKER}{}",
            base64::engine::general_purpose::STANDARD.encode(value.as_bytes())
        )
    } else {
        value.to_string()
    }
}

/// True when `value` would not survive Infisical verbatim.
fn needs_encoding(value: &str) -> bool {
    value.starts_with(MARKER) || value != value.trim() || value.chars().any(|c| c.is_control())
}

/// Decodes a value read from Infisical (inverse of [`encode`]).
pub fn decode(stored: &str) -> Result<String, Box<dyn std::error::Error>> {
    let Some(b64) = stored.strip_prefix(MARKER) else {
        return Ok(stored.to_string());
    };
    use base64::Engine as _;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64.trim())
        .map_err(|e| format!("invalid {MARKER} value: {e}"))?;
    String::from_utf8(bytes).map_err(|_| "decoded value is not UTF-8".into())
}

/// Lower-case hex SHA-256 of `data` (used for in-memory verification only).
pub fn sha256_hex(data: &[u8]) -> String {
    use sha2::Digest as _;
    sha2::Sha256::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[cfg(test)]
#[path = "codec_tests.rs"]
mod tests;
