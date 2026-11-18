//! Infisical backend (REST API v4 secrets, v2 folders, v1 Universal Auth).

use super::config::InfisicalSettings;
use super::{codec, BoxError, SecretMap, SecretsBackend};

/// Infisical REST client.
pub struct InfisicalBackend {
    #[allow(dead_code)]
    settings: InfisicalSettings,
}

impl InfisicalBackend {
    /// Creates a client.
    pub fn new(settings: InfisicalSettings) -> Self {
        Self { settings }
    }
    /// Base delay for 429 retries (tests use 1 ms).
    pub fn with_retry_delay_ms(self, _ms: u64) -> Self {
        todo!()
    }
}

impl SecretsBackend for InfisicalBackend {
    fn name(&self) -> &'static str {
        "infisical"
    }
    fn get(&self, _path: &str) -> Result<Option<SecretMap>, BoxError> {
        let _ = codec::MARKER;
        todo!()
    }
    fn list(&self, _path: &str) -> Result<Vec<String>, BoxError> {
        todo!()
    }
    fn put(&self, _path: &str, _data: &SecretMap) -> Result<(), BoxError> {
        todo!()
    }
    fn location_id(&self, _path: &str) -> Result<String, BoxError> {
        todo!()
    }
}

#[cfg(test)]
#[path = "infisical_tests.rs"]
mod tests;
