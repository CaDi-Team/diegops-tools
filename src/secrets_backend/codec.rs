//! Value codec for backends that cannot store arbitrary strings verbatim.
//!
//! Infisical trims secret values server-side, so multi-line values (PEM/GPG
//! keys), values with surrounding whitespace and binary payloads are stored as
//! `diegops:b64:<base64>`. Plain single-line values are stored verbatim so they
//! stay readable in the Infisical UI.

/// Prefix marking a base64-encoded value.
pub const MARKER: &str = "diegops:b64:";

/// Encodes `value` for storage in Infisical.
pub fn encode(_value: &str) -> String {
    todo!()
}

/// Decodes a value read from Infisical (inverse of [`encode`]).
pub fn decode(_stored: &str) -> Result<String, Box<dyn std::error::Error>> {
    todo!()
}

/// Lower-case hex SHA-256 of `data` (used for in-memory verification only).
pub fn sha256_hex(_data: &[u8]) -> String {
    todo!()
}

#[cfg(test)]
#[path = "codec_tests.rs"]
mod tests;
