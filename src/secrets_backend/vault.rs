//! Vault / OpenBao KV v2 backend over HTTP (no `vault` CLI needed).
//!
//! Address, namespace (`X-Vault-Namespace`), CA certificate and token all come
//! from config/env (see [`super::config::resolve_vault`]); nothing is
//! hard-coded. OIDC users log in with `bao login -method=oidc` (or `vault`)
//! and the resulting `~/.vault-token` is picked up.

use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};
use ureq::http;
use ureq::tls::{Certificate, RootCerts, TlsConfig};

use super::config::VaultSettings;
use super::{BoxError, SecretMap, SecretsBackend};

/// Vault-compatible KV v2 client.
pub struct VaultBackend {
    settings: VaultSettings,
    agent: ureq::Agent,
}

impl VaultBackend {
    /// Creates a client (loads the optional CA certificate).
    pub fn new(settings: VaultSettings) -> Result<Self, BoxError> {
        let mut builder = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(30)))
            .user_agent(format!("diegops/{}", env!("CARGO_PKG_VERSION")))
            .allow_non_standard_methods(true); // LIST
        if let Some(ca) = &settings.ca_cert {
            builder = builder.tls_config(tls_with_ca(ca)?);
        }
        Ok(Self {
            settings,
            agent: builder.build().new_agent(),
        })
    }

    /// Sends `method` to `/v1/<api_path>`, with an optional JSON body.
    fn request(
        &self,
        method: &str,
        api_path: &str,
        body: Option<&Value>,
    ) -> Result<http::Response<ureq::Body>, ureq::Error> {
        let mut req = http::Request::builder()
            .method(http::Method::from_bytes(method.as_bytes()).map_err(http::Error::from)?)
            .uri(format!("{}/v1/{api_path}", self.settings.address))
            .header("X-Vault-Token", &self.settings.token);
        if let Some(ns) = &self.settings.namespace {
            req = req.header("X-Vault-Namespace", ns);
        }
        match body {
            Some(b) => {
                let bytes = serde_json::to_vec(b).map_err(ureq::Error::from)?;
                self.agent
                    .run(req.header("Content-Type", "application/json").body(bytes)?)
            }
            None => self.agent.run(req.body(())?),
        }
    }
}

/// Splits `<mount>/<path>`; `path` may be empty only when `allow_root`.
fn split(path: &str, allow_root: bool) -> Result<(&str, &str), BoxError> {
    let trimmed = path.trim_matches('/');
    let (mount, rest) = trimmed.split_once('/').unwrap_or((trimmed, ""));
    if mount.is_empty() || (rest.is_empty() && !allow_root) {
        return Err(format!("'{path}' is not a <mount>/<path> secret path").into());
    }
    Ok((mount, rest))
}

/// Maps a ureq error to a message without the response body.
fn http_err(op: &str, path: &str, e: ureq::Error) -> BoxError {
    match e {
        ureq::Error::StatusCode(code) => format!("Vault {op} '{path}' failed: HTTP {code}").into(),
        other => format!("Vault {op} '{path}' failed: {other}").into(),
    }
}

impl SecretsBackend for VaultBackend {
    fn name(&self) -> &'static str {
        "vault"
    }

    fn get(&self, path: &str) -> Result<Option<SecretMap>, BoxError> {
        let (mount, rest) = split(path, false)?;
        let v: Value = match self.request("GET", &format!("{mount}/data/{rest}"), None) {
            Ok(mut r) => r.body_mut().read_json()?,
            Err(ureq::Error::StatusCode(404)) => return Ok(None),
            Err(e) => return Err(http_err("read", path, e)),
        };
        let Some(data) = v.pointer("/data/data").and_then(Value::as_object) else {
            return Ok(None);
        };
        Ok(Some(
            data.iter()
                .map(|(k, v)| {
                    let s = match v {
                        Value::String(s) => s.clone(),
                        other => other.to_string(),
                    };
                    (k.clone(), s)
                })
                .collect(),
        ))
    }

    fn list(&self, path: &str) -> Result<Vec<String>, BoxError> {
        let (mount, rest) = split(path, true)?;
        let v: Value = match self.request("LIST", &format!("{mount}/metadata/{rest}"), None) {
            Ok(mut r) => r.body_mut().read_json()?,
            Err(ureq::Error::StatusCode(404)) => return Ok(Vec::new()),
            Err(e) => return Err(http_err("list", path, e)),
        };
        Ok(v.pointer("/data/keys")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect())
    }

    fn put(&self, path: &str, data: &SecretMap) -> Result<(), BoxError> {
        let (mount, rest) = split(path, false)?;
        // KV v2 writes replace the whole secret: merge to keep other keys.
        let mut merged = self.get(path)?.unwrap_or_default();
        merged.extend(data.iter().map(|(k, v)| (k.clone(), v.clone())));
        self.request(
            "POST",
            &format!("{mount}/data/{rest}"),
            Some(&json!({ "data": merged })),
        )
        .map_err(|e| http_err("write", path, e))?;
        Ok(())
    }
}

/// rustls config trusting the bundled web roots plus `ca` (PEM).
fn tls_with_ca(ca: &std::path::Path) -> Result<TlsConfig, BoxError> {
    use rustls_pki_types::pem::PemObject as _;
    use rustls_pki_types::CertificateDer;
    let mut roots = rustls::RootCertStore::empty();
    let mut certs: Vec<Certificate<'static>> = webpki_root_certs::TLS_SERVER_ROOT_CERTS
        .iter()
        .map(|c| Certificate::from_der(c.as_ref()))
        .collect();
    let mut added = 0;
    for cert in CertificateDer::pem_file_iter(ca)
        .map_err(|e| format!("could not read CA cert {}: {e}", ca.display()))?
    {
        let cert = cert.map_err(|e| format!("invalid CA cert {}: {e}", ca.display()))?;
        // Validate like before (rustls rejects unusable trust anchors).
        roots.add(cert.clone())?;
        certs.push(Certificate::from_der(cert.as_ref()).to_owned());
        added += 1;
    }
    if added == 0 {
        return Err(format!("no certificates found in {}", ca.display()).into());
    }
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    Ok(TlsConfig::builder()
        .root_certs(RootCerts::new_with_certs(&certs))
        .unversioned_rustls_crypto_provider(provider)
        .build())
}

#[cfg(test)]
#[path = "vault_tests.rs"]
mod tests;
