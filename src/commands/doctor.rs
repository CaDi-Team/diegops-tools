//! `diegops doctor`: checks this machine's setup and says how to fix each problem.

use crate::secrets_backend::{self, config};
use serde::Serialize;
use std::path::Path;

type BoxError = Box<dyn std::error::Error>;

const GITHUB_API: &str = "https://api.github.com";

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Ok,
    Warn,
    Fail,
    Skip,
}

#[derive(Debug, Serialize)]
pub struct Check {
    pub name: String,
    pub status: Status,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

impl Check {
    fn new(name: &str, status: Status, detail: impl Into<String>) -> Self {
        Check {
            name: name.to_string(),
            status,
            detail: detail.into(),
            hint: None,
        }
    }
    fn hint(mut self, h: impl Into<String>) -> Self {
        self.hint = Some(h.into());
        self
    }
}

/// YAML configs: (file, env override, init command, what it is for).
pub(crate) const YAML_CONFIGS: &[(&str, &str, &str)] = &[
    ("repos.yaml", "DIEGOPS_REPOS_CONFIG", "repos to clone"),
    (
        "secrets.yaml",
        "DIEGOPS_SECRETS_CONFIG",
        "secret files to back up",
    ),
    (
        "repo-vault.yaml",
        "DIEGOPS_VAULT_CONFIG",
        ".env files to write",
    ),
    (
        "agents.yaml",
        "DIEGOPS_AGENTS_CONFIG",
        "AI agents to back up",
    ),
];

pub fn run(json: bool, offline: bool) -> Result<(), BoxError> {
    let checks = collect(offline, GITHUB_API);
    let ok = !checks.iter().any(|c| c.status == Status::Fail);
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({ "ok": ok, "checks": checks }))?
        );
    } else {
        print_human(&checks, offline);
    }
    if !ok {
        std::process::exit(1);
    }
    Ok(())
}

fn print_human(checks: &[Check], offline: bool) {
    println!(
        "diegops doctor{}\n",
        if offline { " (offline)" } else { "" }
    );
    let w = checks.iter().map(|c| c.name.len()).max().unwrap_or(0);
    for c in checks {
        let mark = match c.status {
            Status::Ok => '✓',
            Status::Warn => '!',
            Status::Fail => '✗',
            Status::Skip => '-',
        };
        println!("  {mark} {:<w$}  {}", c.name, c.detail);
        if let Some(h) = &c.hint {
            for line in h.lines() {
                println!("    {:<w$}  hint: {line}", "");
            }
        }
    }
    let fails = checks.iter().filter(|c| c.status == Status::Fail).count();
    let warns = checks.iter().filter(|c| c.status == Status::Warn).count();
    println!("\n{fails} problem(s), {warns} warning(s). Terms: diegops explain");
}

/// Runs every check. `github_api` is a parameter so tests can mock it.
pub fn collect(offline: bool, github_api: &str) -> Vec<Check> {
    let mut out = vec![binary_check()];
    out.extend(yaml_checks());
    out.extend(backend_checks(offline));
    out.extend(github_checks(offline, github_api));
    out.push(tools_check());
    out
}

fn binary_check() -> Check {
    let version = format!("diegops v{}", env!("CARGO_PKG_VERSION"));
    let exe = std::env::current_exe().ok();
    let on_path = exe
        .as_ref()
        .and_then(|e| e.file_name())
        .is_some_and(|name| {
            std::env::var_os("PATH")
                .map(|p| std::env::split_paths(&p).any(|d| d.join(name).exists()))
                .unwrap_or(false)
        });
    let where_ = exe.map(|e| e.display().to_string()).unwrap_or_default();
    if on_path {
        Check::new("binary", Status::Ok, format!("{version} ({where_})"))
    } else {
        Check::new(
            "binary",
            Status::Warn,
            format!("{version} not on PATH ({where_})"),
        )
        .hint("add ~/.local/bin to PATH, or reinstall with the install script")
    }
}

fn yaml_checks() -> Vec<Check> {
    YAML_CONFIGS
        .iter()
        .map(|(file, env, what)| {
            let path = match super::common::resolve_config_path(None, env, file) {
                Ok(p) => p,
                Err(e) => return Check::new(file, Status::Fail, e.to_string()),
            };
            let init = super::common::init_command_for(&path)
                .map(|c| format!("run '{c}' to create a sample"))
                .unwrap_or_default();
            match std::fs::read_to_string(&path) {
                Err(_) => Check::new(file, Status::Warn, format!("not found ({what})"))
                    .hint(format!("{init} (only needed if you use it)")),
                Ok(s) => match serde_norway::from_str::<serde_norway::Value>(&s) {
                    Ok(_) => Check::new(file, Status::Ok, path.display().to_string()),
                    Err(e) => Check::new(file, Status::Fail, format!("invalid YAML: {e}"))
                        .hint(format!("fix {}", path.display())),
                },
            }
        })
        .collect()
}

fn backend_checks(offline: bool) -> Vec<Check> {
    let env = secrets_backend::real_env;
    let home = secrets_backend::home();
    let path = config::default_path(&env, &home);
    let mut out = Vec::new();
    let cfg = match config::load(&path) {
        Ok(c) => {
            let note = if path.exists() {
                path.display().to_string()
            } else {
                format!("{} (not found, using defaults)", path.display())
            };
            out.push(Check::new("config.toml", Status::Ok, note));
            c
        }
        Err(e) => {
            out.push(
                Check::new("config.toml", Status::Fail, e.to_string())
                    .hint(format!("fix {}", path.display())),
            );
            return out;
        }
    };
    let kind = match config::select_kind(&cfg, None, &env) {
        Ok(k) => k,
        Err(e) => {
            out.push(Check::new("backend", Status::Fail, e.to_string()));
            return out;
        }
    };
    let name = match kind {
        config::BackendKind::Infisical => "infisical",
        config::BackendKind::Vault => "vault",
    };
    if let Some(f) = &cfg.infisical.client_secret_file {
        out.push(secret_file_check(&config::expand(f, &home)));
    }
    let source = config::credential_source(&cfg, kind, &env, &home);
    match secrets_backend::open_kind(&cfg, kind) {
        Err(e) => {
            let first = e.to_string().lines().next().unwrap_or_default().to_string();
            let hint = match kind {
                config::BackendKind::Infisical => {
                    "see 'diegops explain machine-identity' and 'diegops kv --help'"
                }
                config::BackendKind::Vault => {
                    "run 'bao login' or set VAULT_TOKEN; see 'diegops explain vault'"
                }
            };
            out.push(Check::new("backend", Status::Fail, format!("{name}: {first}")).hint(hint));
        }
        Ok(backend) => {
            let c = if offline {
                Check::new(
                    "backend",
                    Status::Ok,
                    format!("{name}: credentials found via {source} (not contacted: --offline)"),
                )
            } else {
                match backend.ping() {
                    Ok(()) => Check::new(
                        "backend",
                        Status::Ok,
                        format!("{name}: credentials accepted ({source})"),
                    ),
                    Err(e) => Check::new("backend", Status::Fail, format!("{name}: {e}"))
                        .hint("check the credentials and that the server is reachable; 'diegops explain backend'"),
                }
            };
            out.push(c);
        }
    }
    out
}

/// Client secret files must be private (0600).
pub fn secret_file_check(path: &Path) -> Check {
    private_file_check("client secret", path)
}

/// A credential file must be private (0600): ADR 0001 D6 file store.
pub fn private_file_check(name: &'static str, path: &Path) -> Check {
    let meta = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return Check::new(name, Status::Fail, format!("{} not found", path.display()))
            .hint(
            "create it with the Client Secret, mode 600; see 'diegops explain machine-identity'",
        ),
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = meta.permissions().mode() & 0o777;
        if mode & 0o077 != 0 {
            return Check::new(
                name,
                Status::Warn,
                format!("{} is readable by others (mode {mode:o})", path.display()),
            )
            .hint(format!("chmod 600 {}", path.display()));
        }
    }
    let _ = meta;
    Check::new(name, Status::Ok, format!("{} (private)", path.display()))
}

fn github_checks(offline: bool, api: &str) -> Vec<Check> {
    let (token, source) = match super::auth::load_gh_token_with_source() {
        Ok(Some(t)) => t,
        Ok(None) => {
            return vec![Check::new(
                "github token",
                Status::Fail,
                "not set (needed by sync and private repos)",
            )
            .hint("run 'diegops auth gh login'; see 'diegops explain pat'")]
        }
        Err(e) => return vec![Check::new("github token", Status::Fail, e.to_string())],
    };
    let mut out = Vec::new();
    if source == super::auth::GhTokenSource::Stored {
        if let Ok(dir) = super::auth::tokens_dir() {
            out.push(private_file_check(
                "github token file",
                &dir.join("gh.json"),
            ));
        }
    }
    if offline {
        out.push(Check::new(
            "github token",
            Status::Ok,
            format!("{} (not validated: --offline)", source.label()),
        ));
        return out;
    }
    out.extend(github_token_check(api, &token).into_iter().map(|mut c| {
        if c.name == "github token" && c.status == Status::Ok {
            c.detail = format!("{} ({})", c.detail, source.label());
        }
        c
    }));
    out
}

/// Validates the token and looks for the sync repo.
pub fn github_token_check(api: &str, token: &str) -> Vec<Check> {
    let ua = format!("diegops/{}", env!("CARGO_PKG_VERSION"));
    let get = |url: String| {
        ureq::get(&url)
            .header("User-Agent", &ua)
            .header("Authorization", &format!("Bearer {token}"))
            .header("Accept", "application/vnd.github+json")
            .call()
    };
    let login = match get(format!("{api}/user")) {
        Ok(mut r) => {
            let v: serde_json::Value = r.body_mut().read_json().unwrap_or_default();
            v["login"].as_str().unwrap_or("?").to_string()
        }
        Err(ureq::Error::StatusCode(401 | 403)) => {
            return vec![Check::new(
                "github token",
                Status::Fail,
                "rejected by GitHub (invalid or expired)",
            )
            .hint("create a new one and run 'diegops auth gh login'")]
        }
        Err(e) => {
            return vec![Check::new(
                "github token",
                Status::Warn,
                format!("could not reach GitHub: {e}"),
            )
            .hint("check your connection, or rerun with --offline")]
        }
    };
    let mut out = vec![Check::new(
        "github token",
        Status::Ok,
        format!("valid, user {login}"),
    )];
    let repo = format!("diegops-{login}-memory");
    out.push(match get(format!("{api}/repos/{login}/{repo}")) {
        Ok(_) => Check::new("sync repo", Status::Ok, format!("{login}/{repo}")),
        Err(ureq::Error::StatusCode(404)) => Check::new(
            "sync repo",
            Status::Warn,
            format!("{login}/{repo} not found"),
        )
        .hint("run 'diegops sync push' to create it (only needed if you use sync)"),
        Err(e) => Check::new("sync repo", Status::Warn, format!("could not check: {e}")),
    });
    out
}

fn tools_check() -> Check {
    let bin = match super::common::diegops_dir() {
        Ok(d) => d.join("bin"),
        Err(e) => return Check::new("managed tools", Status::Fail, e.to_string()),
    };
    let count = std::fs::read_dir(&bin).map(|d| d.count()).unwrap_or(0);
    let on_path = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| d == bin))
        .unwrap_or(false);
    match (count, on_path) {
        (0, _) => Check::new("managed tools", Status::Skip, "none installed")
            .hint("optional: 'diegops tool list' shows what can be installed"),
        (n, true) => Check::new(
            "managed tools",
            Status::Ok,
            format!("{n} in {}", bin.display()),
        ),
        (n, false) => Check::new(
            "managed tools",
            Status::Warn,
            format!("{n} in {}, which is not on PATH", bin.display()),
        )
        .hint("run 'diegops shell init' or add ~/.diegops/bin to PATH"),
    }
}

#[cfg(test)]
#[path = "doctor_tests.rs"]
mod tests;
