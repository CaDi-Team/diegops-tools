//! Vault / OpenBao KV v2 backend over HTTP (no `vault` CLI needed).

use super::config::VaultSettings;
use super::{BoxError, SecretMap, SecretsBackend};

/// Vault-compatible KV v2 client.
pub struct VaultBackend {
    #[allow(dead_code)]
    settings: VaultSettings,
}

impl VaultBackend {
    /// Creates a client (loads the optional CA certificate).
    pub fn new(settings: VaultSettings) -> Result<Self, BoxError> {
        Ok(Self { settings })
    }
}

impl SecretsBackend for VaultBackend {
    fn name(&self) -> &'static str {
        "vault"
    }
    fn get(&self, _path: &str) -> Result<Option<SecretMap>, BoxError> {
        todo!()
    }
    fn list(&self, _path: &str) -> Result<Vec<String>, BoxError> {
        todo!()
    }
    fn put(&self, _path: &str, _data: &SecretMap) -> Result<(), BoxError> {
        todo!()
    }
}

#[cfg(test)]
#[path = "vault_tests.rs"]
mod tests;
