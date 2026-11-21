//! Infisical backend (REST API v4 secrets, v2 folders, v1 Universal Auth).
//!
//! Values go through [`codec`] because Infisical trims them server-side.
//! Error messages carry the HTTP status and Infisical's message only for
//! non-write requests; write errors never echo the server body.

use std::cell::RefCell;
use std::time::Duration;

use serde_json::{json, Value};
use ureq::http;

use super::config::{InfisicalAuth, InfisicalSettings, Location};
use super::{codec, BoxError, SecretMap, SecretsBackend};

/// Maximum attempts for a rate-limited (429) or 5xx request.
const MAX_ATTEMPTS: u32 = 6;

/// Infisical REST client.
pub struct InfisicalBackend {
    settings: InfisicalSettings,
    agent: ureq::Agent,
    token: RefCell<Option<String>>,
    retry_delay_ms: u64,
}

impl InfisicalBackend {
    /// Creates a client.
    pub fn new(settings: InfisicalSettings) -> Self {
        // Status errors are handled in `send` (body message, Retry-After).
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(30)))
            .user_agent(format!("diegops/{}", env!("CARGO_PKG_VERSION")))
            .http_status_as_error(false)
            .build()
            .new_agent();
        Self {
            settings,
            agent,
            token: RefCell::new(None),
            retry_delay_ms: 1000,
        }
    }

    /// Base delay for 429 retries (tests use 1 ms).
    #[cfg(test)]
    pub fn with_retry_delay_ms(mut self, ms: u64) -> Self {
        self.retry_delay_ms = ms;
        self
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.settings.site_url)
    }

    /// Returns a bearer token, logging in with Universal Auth once.
    fn token(&self) -> Result<String, BoxError> {
        if let Some(t) = self.token.borrow().as_ref() {
            return Ok(t.clone());
        }
        let t = match &self.settings.auth {
            InfisicalAuth::AccessToken(t) => t.clone(),
            InfisicalAuth::UniversalAuth {
                client_id,
                client_secret,
            } => {
                let body = json!({ "clientId": client_id, "clientSecret": client_secret });
                let resp = self
                    .send(|| {
                        json_body(
                            self.agent
                                .post(self.url("/api/v1/auth/universal-auth/login")),
                            &body,
                        )
                    })
                    .map_err(|e| format!("Infisical login failed: {e}"))?;
                resp.get("accessToken")
                    .and_then(Value::as_str)
                    .ok_or("Infisical login returned no accessToken")?
                    .to_string()
            }
        };
        *self.token.borrow_mut() = Some(t.clone());
        Ok(t)
    }

    /// Sends a request with retries; returns parsed JSON or `HttpError`.
    fn send(
        &self,
        run: impl Fn() -> Result<http::Response<ureq::Body>, ureq::Error>,
    ) -> Result<Value, HttpError> {
        let mut attempt = 0;
        loop {
            attempt += 1;
            match run() {
                Ok(mut r) if r.status().as_u16() < 400 => {
                    let text = r.body_mut().read_to_string().unwrap_or_default();
                    return Ok(serde_json::from_str(&text).unwrap_or(Value::Null));
                }
                Ok(mut r) => {
                    let code = r.status().as_u16();
                    let retryable = code == 429 || code >= 500;
                    if retryable && attempt < MAX_ATTEMPTS {
                        let wait = r
                            .headers()
                            .get("retry-after")
                            .and_then(|v| v.to_str().ok())
                            .and_then(|s| s.parse::<u64>().ok())
                            .map(|s| s * 1000)
                            .unwrap_or(self.retry_delay_ms << (attempt - 1))
                            .min(60_000)
                            .max(self.retry_delay_ms);
                        std::thread::sleep(Duration::from_millis(wait));
                        continue;
                    }
                    let message = r
                        .body_mut()
                        .read_json::<Value>()
                        .ok()
                        .and_then(|v| v.get("message").and_then(Value::as_str).map(str::to_string))
                        .unwrap_or_default();
                    return Err(HttpError::Status(code, message));
                }
                Err(e) => return Err(HttpError::Transport(e.to_string())),
            }
        }
    }

    /// POST/PATCH `path` with a bearer token and a JSON body.
    fn send_authed(
        &self,
        method: &str,
        path: &str,
        body: &Value,
    ) -> Result<Result<Value, HttpError>, BoxError> {
        let t = self.token()?;
        let url = self.url(path);
        Ok(self.send(|| {
            let req = match method {
                "PATCH" => self.agent.patch(&url),
                _ => self.agent.post(&url),
            };
            json_body(req.header("Authorization", format!("Bearer {t}")), body)
        }))
    }

    /// Raw (still encoded) values at a location; `None` if the folder is missing.
    fn fetch_raw(&self, loc: &Location) -> Result<Option<SecretMap>, BoxError> {
        let token = self.token()?;
        let res = self.send(|| {
            self.agent
                .get(self.url("/api/v4/secrets"))
                .header("Authorization", format!("Bearer {token}"))
                .query("projectId", &loc.project_id)
                .query("environment", &loc.environment)
                .query("secretPath", &loc.folder)
                .query("viewSecretValue", "true")
                .query("expandSecretReferences", "false")
                .query("recursive", "false")
                .query("includeImports", "false")
                .call()
        });
        let v = match res {
            Ok(v) => v,
            Err(HttpError::Status(404, _)) => return Ok(None),
            Err(e) => return Err(format!("Infisical read {} failed: {e}", describe(loc)).into()),
        };
        let mut out = SecretMap::new();
        for s in v
            .get("secrets")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let (Some(k), Some(val)) = (
                s.get("secretKey").and_then(Value::as_str),
                s.get("secretValue").and_then(Value::as_str),
            ) {
                out.insert(k.to_string(), val.to_string());
            }
        }
        Ok(if out.is_empty() { None } else { Some(out) })
    }

    /// Creates the folder (and parents); "already exists" is success.
    fn ensure_folder(&self, loc: &Location) -> Result<(), BoxError> {
        let Some((parent, name)) = loc.folder.rsplit_once('/') else {
            return Ok(());
        };
        if name.is_empty() {
            return Ok(());
        }
        let parent = if parent.is_empty() { "/" } else { parent };
        let body = json!({
            "projectId": loc.project_id,
            "environment": loc.environment,
            "name": name,
            "path": parent,
        });
        match self.send_authed("POST", "/api/v2/folders", &body)? {
            Ok(_) => Ok(()),
            Err(HttpError::Status(400, m)) if m.contains("already exists") => Ok(()),
            Err(e) => Err(format!("Infisical create folder {} failed: {e}", describe(loc)).into()),
        }
    }
}

impl SecretsBackend for InfisicalBackend {
    fn ping(&self) -> Result<(), BoxError> {
        self.token().map(|_| ())
    }

    fn name(&self) -> &'static str {
        "infisical"
    }

    fn get(&self, path: &str) -> Result<Option<SecretMap>, BoxError> {
        let loc = self.settings.mapping.locate(path)?;
        let Some(raw) = self.fetch_raw(&loc)? else {
            return Ok(None);
        };
        let mut out = SecretMap::new();
        for (k, v) in raw {
            let decoded = codec::decode(&v).map_err(|e| format!("{path}:{k}: {e}"))?;
            out.insert(k, decoded);
        }
        Ok(Some(out))
    }

    fn list(&self, path: &str) -> Result<Vec<String>, BoxError> {
        let loc = self.settings.mapping.locate(path)?;
        let token = self.token()?;
        let res = self.send(|| {
            self.agent
                .get(self.url("/api/v2/folders"))
                .header("Authorization", format!("Bearer {token}"))
                .query("projectId", &loc.project_id)
                .query("environment", &loc.environment)
                .query("path", &loc.folder)
                .call()
        });
        let v = match res {
            Ok(v) => v,
            Err(HttpError::Status(404, _)) => return Ok(Vec::new()),
            Err(e) => return Err(format!("Infisical list {} failed: {e}", describe(&loc)).into()),
        };
        Ok(v.get("folders")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|f| f.get("name").and_then(Value::as_str))
            .map(|n| format!("{n}/"))
            .collect())
    }

    fn put(&self, path: &str, data: &SecretMap) -> Result<(), BoxError> {
        let loc = self.settings.mapping.locate(path)?;
        self.ensure_folder(&loc)?;
        let current = self.fetch_raw(&loc)?.unwrap_or_default();
        let changed: Vec<Value> = data
            .iter()
            .filter(|(k, v)| {
                current
                    .get(*k)
                    .and_then(|cur| codec::decode(cur).ok())
                    .as_deref()
                    != Some(v.as_str())
            })
            .map(|(k, v)| json!({ "secretKey": k, "secretValue": codec::encode(v) }))
            .collect();
        if changed.is_empty() {
            return Ok(());
        }
        let body = json!({
            "projectId": loc.project_id,
            "environment": loc.environment,
            "secretPath": loc.folder,
            "mode": "upsert",
            "secrets": changed,
        });
        match self.send_authed("PATCH", "/api/v4/secrets/batch", &body)? {
            Ok(_) => Ok(()),
            // Never echo the server message on writes: it may quote values.
            Err(HttpError::Status(code, _)) => {
                Err(format!("Infisical write {} failed: HTTP {code}", describe(&loc)).into())
            }
            Err(e) => Err(format!("Infisical write {} failed: {e}", describe(&loc)).into()),
        }
    }

    fn validate_key(&self, key: &str) -> Result<(), BoxError> {
        if key.is_empty() || key != key.trim() || key.contains(':') || key.contains('/') {
            return Err(format!(
                "key '{key}' is not valid in Infisical (no ':' or '/', no surrounding spaces)"
            )
            .into());
        }
        Ok(())
    }

    fn location_id(&self, path: &str) -> Result<String, BoxError> {
        Ok(describe(&self.settings.mapping.locate(path)?))
    }
}

/// Sends `body` as JSON with the same `Content-Type` ureq 2 used.
fn json_body(
    req: ureq::RequestBuilder<ureq::typestate::WithBody>,
    body: &Value,
) -> Result<http::Response<ureq::Body>, ureq::Error> {
    req.header("Content-Type", "application/json")
        .send_json(body)
}

/// `project/env:/folder`.
fn describe(loc: &Location) -> String {
    format!("{}/{}:{}", loc.project_id, loc.environment, loc.folder)
}

/// HTTP failure without secret material.
#[derive(Debug)]
enum HttpError {
    Status(u16, String),
    Transport(String),
}

impl std::fmt::Display for HttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HttpError::Status(code, m) if m.is_empty() => write!(f, "HTTP {code}"),
            HttpError::Status(code, m) => write!(f, "HTTP {code}: {m}"),
            HttpError::Transport(e) => write!(f, "{e}"),
        }
    }
}

#[cfg(test)]
#[path = "infisical_tests.rs"]
mod tests;
