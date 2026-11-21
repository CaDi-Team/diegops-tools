//! Shared pieces for `diegops agents`: the agent registry
//! (`~/.diegops/agents.yaml`), `.env` parsing/rendering, the systemd user
//! timer units, and the last-run state file.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

type BoxError = Box<dyn std::error::Error>;

/// Supported agent kinds. Only `hermes` is implemented today.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// Hermes Agent (Nous Research): `~/.hermes` + `~/.hermes/profiles/*`.
    Hermes,
    /// pi coding agent (not implemented yet).
    Pi,
    /// opencode (not implemented yet).
    Opencode,
    /// Claude Code (not implemented yet).
    Claude,
}

impl Kind {
    /// CLI / directory name of the kind.
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Hermes => "hermes",
            Kind::Pi => "pi",
            Kind::Opencode => "opencode",
            Kind::Claude => "claude",
        }
    }
}

/// Git repository that receives the agent context.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepoConfig {
    /// Local clone path (`$HOME`/`~` expanded).
    pub path: String,
    /// Clone URL, used when the local clone is missing (fresh PC).
    pub url: Option<String>,
    /// Branch to pull/push (default: the clone's current branch).
    pub branch: Option<String>,
}

/// One registered agent.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentEntry {
    /// Agent kind.
    pub kind: Kind,
    /// Agent home (default per kind, e.g. `~/.hermes`).
    pub root: Option<String>,
    /// Secrets-backend base path for per-profile `.env` (`<base>/<profile>`).
    pub secrets_path: Option<String>,
    /// Secrets backend for this agent (`infisical` | `openbao` | `vault`);
    /// default: the one in `~/.config/diegops/config.toml` (Infisical).
    pub secrets_backend: Option<String>,
}

/// `~/.diegops/agents.yaml`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Registry {
    /// Destination repo.
    pub repo: RepoConfig,
    /// Optional `<path>#<key>` in the secrets backend holding a heartbeat URL
    /// pinged after every successful backup.
    pub heartbeat_kv: Option<String>,
    /// Registered agents.
    #[serde(default)]
    pub agents: Vec<AgentEntry>,
}

impl Registry {
    /// Parses YAML text.
    pub fn parse(text: &str) -> Result<Self, BoxError> {
        serde_norway::from_str(text).map_err(|e| format!("invalid agents config: {e}").into())
    }

    /// Loads from `--config` > `$DIEGOPS_AGENTS_CONFIG` > `~/.diegops/agents.yaml`.
    pub fn load(path: Option<&Path>) -> Result<(PathBuf, Self), BoxError> {
        let p = super::super::common::resolve_config_path(
            path,
            "DIEGOPS_AGENTS_CONFIG",
            "agents.yaml",
        )?;
        let text = super::super::common::read_config_file(&p)?;
        Ok((p.clone(), Self::parse(&text)?))
    }

    /// First entry of `kind`.
    pub fn find(&self, kind: Kind) -> Option<&AgentEntry> {
        self.agents.iter().find(|a| a.kind == kind)
    }
}

/// Expands a leading `~/` or `$HOME`.
pub fn expand(path: &str) -> PathBuf {
    match path.strip_prefix("~/") {
        Some(rest) => super::super::common::expand_home(&format!("$HOME/{rest}")),
        None => super::super::common::expand_home(path),
    }
}

/// Splits `<path>#<key>`.
pub fn split_kv_ref(r: &str) -> Result<(String, String), BoxError> {
    match r.rsplit_once('#') {
        Some((p, k)) if !p.is_empty() && !k.is_empty() => Ok((p.to_string(), k.to_string())),
        _ => Err(format!("'{r}': expected <path>#<key>").into()),
    }
}

// ---------------------------------------------------------------------------
// .env
// ---------------------------------------------------------------------------

fn valid_key(k: &str) -> bool {
    let mut c = k.chars();
    matches!(c.next(), Some(ch) if ch == '_' || ch.is_ascii_alphabetic())
        && c.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

/// Parses a dotenv file (`KEY=v`, `export KEY=v`, single/double quotes).
/// Lines that are not assignments are ignored.
pub fn parse_env(text: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line);
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let k = k.trim();
        if !valid_key(k) {
            continue;
        }
        let v = v.trim();
        let value = if v.len() >= 2 && v.starts_with('"') && v.ends_with('"') {
            let mut s = String::new();
            let mut it = v[1..v.len() - 1].chars();
            while let Some(ch) = it.next() {
                if ch == '\\' {
                    match it.next() {
                        Some('n') => s.push('\n'),
                        Some(o) => s.push(o),
                        None => s.push('\\'),
                    }
                } else {
                    s.push(ch);
                }
            }
            s
        } else if v.len() >= 2 && v.starts_with('\'') && v.ends_with('\'') {
            v[1..v.len() - 1].to_string()
        } else {
            v.to_string()
        };
        out.insert(k.to_string(), value);
    }
    out
}

/// Renders a dotenv file with double-quoted, escaped values.
pub fn render_env(map: &BTreeMap<String, String>) -> String {
    let mut s =
        String::from("# Generated by diegops agents from the secrets backend — do not commit\n");
    for (k, v) in map {
        let esc = v
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n");
        s.push_str(&format!("{k}=\"{esc}\"\n"));
    }
    s
}

// ---------------------------------------------------------------------------
// systemd user timer
// ---------------------------------------------------------------------------

/// Unit base name.
pub const UNIT: &str = "diegops-agents-backup";

/// Renders `(service, timer)` unit files; one `ExecStart` per agent kind so
/// the journal names which agent failed.
pub fn render_units(exe: &Path, kinds: &[Kind]) -> (String, String) {
    let mut svc = String::from(
        "# Managed by `diegops agents install-timer` — re-run it instead of editing.\n\
         [Unit]\n\
         Description=diegops: back up local AI agent context\n\
         Wants=network-online.target\n\
         After=network-online.target\n\n\
         [Service]\n\
         Type=oneshot\n",
    );
    for k in kinds {
        svc.push_str(&format!(
            "ExecStart={} agents {} backup\n",
            exe.display(),
            k.as_str()
        ));
    }
    let tmr = "# Managed by `diegops agents install-timer`.\n\
         [Unit]\n\
         Description=Daily diegops agent context backup\n\n\
         [Timer]\n\
         OnCalendar=daily\n\
         RandomizedDelaySec=30min\n\
         Persistent=true\n\n\
         [Install]\n\
         WantedBy=timers.target\n"
        .to_string();
    (svc, tmr)
}

// ---------------------------------------------------------------------------
// Run state (~/.local/state/diegops/agents.json)
// ---------------------------------------------------------------------------

/// Last-run record of one agent.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentRun {
    /// Unix time of the last successful backup.
    pub last_ok: Option<u64>,
    /// Unix time of the last attempt.
    pub last_attempt: u64,
    /// Error of the last attempt, if it failed.
    pub last_error: Option<String>,
    /// Files in the last successful backup.
    pub files: usize,
    /// Bytes in the last successful backup.
    pub bytes: u64,
    /// Repo commit after the last successful backup.
    pub commit: Option<String>,
}

/// All agents' last-run records.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RunState {
    /// Keyed by agent kind name.
    pub agents: BTreeMap<String, AgentRun>,
}

impl RunState {
    /// Records an attempt: `Ok((files, bytes, commit))` or `Err(message)`.
    pub fn record(&mut self, agent: &str, r: Result<(usize, u64, String), String>, now: u64) {
        let e = self.agents.entry(agent.to_string()).or_default();
        e.last_attempt = now;
        match r {
            Ok((files, bytes, commit)) => {
                e.last_ok = Some(now);
                e.last_error = None;
                e.files = files;
                e.bytes = bytes;
                e.commit = Some(commit);
            }
            Err(msg) => e.last_error = Some(msg),
        }
    }

    /// State file path (`$XDG_STATE_HOME` or `~/.local/state`).
    pub fn path() -> Result<PathBuf, BoxError> {
        let base = match std::env::var_os("XDG_STATE_HOME") {
            Some(p) if !p.is_empty() => PathBuf::from(p),
            _ => super::super::common::home_dir()?
                .join(".local")
                .join("state"),
        };
        Ok(base.join("diegops").join("agents.json"))
    }

    /// Parses saved state; a corrupt file yields empty state plus a warning.
    pub fn parse(text: &str, path: &Path) -> (Self, Option<String>) {
        match serde_json::from_str(text) {
            Ok(s) => (s, None),
            Err(e) => (
                Self::default(),
                Some(format!(
                    "agents state file {} is corrupt ({e}); last-run info starts empty and will be rewritten on the next backup",
                    path.display()
                )),
            ),
        }
    }

    /// Loads the state (empty when missing).
    pub fn load() -> Result<Self, BoxError> {
        match std::fs::read_to_string(Self::path()?) {
            Ok(t) => {
                let (s, warn) = Self::parse(&t, &Self::path()?);
                if let Some(w) = warn {
                    eprintln!("warning: {w}");
                }
                Ok(s)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }

    /// Saves the state.
    pub fn save(&self) -> Result<(), BoxError> {
        let p = Self::path()?;
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d)?;
        }
        std::fs::write(p, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }
}

/// Current Unix time in seconds.
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Compact age like `3h2m` or `2d0h`.
pub fn human_age(from: u64, now: u64) -> String {
    let s = now.saturating_sub(from);
    if s >= 86_400 {
        format!("{}d{}h", s / 86_400, (s % 86_400) / 3600)
    } else {
        format!("{}h{}m", s / 3600, (s % 3600) / 60)
    }
}

#[cfg(test)]
#[path = "support_tests.rs"]
mod tests;
