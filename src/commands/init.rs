//! `diegops init`: guided first run (ADR 0001 D5).
//!
//! Orchestrates what already exists (config, backend login, GitHub token,
//! `doctor`); it does not duplicate their logic. Every prompt has a flag and
//! an env var ([`FIELDS`]), so `--no-input` runs the same path.

use std::path::{Path, PathBuf};

use crate::secrets_backend::config::{self, BackendKind, Config};

type BoxError = Box<dyn std::error::Error>;

/// One wizard field and its non-interactive equivalents.
pub struct Field {
    pub key: &'static str,
    pub label: &'static str,
    /// Long flag (without `--`) of `diegops init`.
    pub flag: &'static str,
    /// Env var read by that flag.
    pub env: &'static str,
    /// One-line "why", shown above the prompt.
    pub why: &'static str,
}

/// Every wizard field. A test checks each has its flag and env var in clap.
pub const FIELDS: &[Field] = &[
    Field {
        key: "backend",
        label: "Secrets backend",
        flag: "backend",
        env: "DIEGOPS_SECRETS_BACKEND",
        why: "where .env values and secret files live (see 'diegops explain backend')",
    },
    Field {
        key: "site_url",
        label: "Infisical URL",
        flag: "site-url",
        env: "INFISICAL_API_URL",
        why: "Infisical Cloud US unless you self-host",
    },
    Field {
        key: "project_id",
        label: "Infisical project ID",
        flag: "project-id",
        env: "DIEGOPS_INFISICAL_PROJECT_ID",
        why: "Project Settings → Project ID; the default project for your secrets",
    },
    Field {
        key: "client_id",
        label: "Machine identity Client ID",
        flag: "client-id",
        env: "INFISICAL_UNIVERSAL_AUTH_CLIENT_ID",
        why: "Universal Auth identity diegops logs in as (see 'diegops explain machine-identity')",
    },
    Field {
        key: "client_secret",
        label: "Machine identity Client Secret",
        flag: "client-secret-file",
        env: "INFISICAL_UNIVERSAL_AUTH_CLIENT_SECRET",
        why: "stored in a private file (mode 600), never in config.toml",
    },
    Field {
        key: "vault_addr",
        label: "Vault/OpenBao address",
        flag: "vault-addr",
        env: "VAULT_ADDR",
        why: "token comes from VAULT_TOKEN or 'bao login' (~/.vault-token)",
    },
];

fn field(key: &str) -> &'static Field {
    FIELDS.iter().find(|f| f.key == key).expect("known field")
}

/// Values given up front (flags or env, resolved by clap).
#[derive(Debug, Default, Clone)]
pub struct Given {
    pub backend: Option<String>,
    pub site_url: Option<String>,
    pub project_id: Option<String>,
    pub client_id: Option<String>,
    pub client_secret_file: Option<PathBuf>,
    pub client_secret: Option<String>,
    pub vault_addr: Option<String>,
    pub skip_github: bool,
    pub reset: Vec<String>,
}

/// How the wizard asks. `NoPrompt` fails with the flag/env to use instead.
pub trait Prompter {
    fn text(&mut self, f: &Field, default: Option<&str>) -> Result<String, BoxError>;
    fn secret(&mut self, f: &Field) -> Result<String, BoxError>;
    fn select(&mut self, f: &Field, options: &[&str], default: usize) -> Result<usize, BoxError>;
}

/// `--no-input` / no terminal: every missing value is an error with a hint.
pub struct NoPrompt;

pub fn missing(f: &Field) -> BoxError {
    format!(
        "{} is needed and there is no terminal to ask in\n  hint: pass --{} or set {}",
        f.label, f.flag, f.env
    )
    .into()
}

impl Prompter for NoPrompt {
    fn text(&mut self, f: &Field, default: Option<&str>) -> Result<String, BoxError> {
        default.map(str::to_string).ok_or_else(|| missing(f))
    }
    fn secret(&mut self, f: &Field) -> Result<String, BoxError> {
        Err(missing(f))
    }
    fn select(&mut self, _f: &Field, _o: &[&str], default: usize) -> Result<usize, BoxError> {
        Ok(default)
    }
}

/// Interactive prompts (inquire).
pub struct Inquire;

impl Prompter for Inquire {
    fn text(&mut self, f: &Field, default: Option<&str>) -> Result<String, BoxError> {
        let msg = format!("{}:", f.label);
        let mut p = inquire::Text::new(&msg).with_help_message(f.why);
        if let Some(d) = default {
            p = p.with_default(d);
        }
        Ok(p.prompt()?.trim().to_string())
    }
    fn secret(&mut self, f: &Field) -> Result<String, BoxError> {
        Ok(inquire::Password::new(&format!("{}:", f.label))
            .with_help_message(f.why)
            .without_confirmation()
            .with_display_mode(inquire::PasswordDisplayMode::Masked)
            .prompt()?
            .trim()
            .to_string())
    }
    fn select(&mut self, f: &Field, options: &[&str], default: usize) -> Result<usize, BoxError> {
        let choice = inquire::Select::new(&format!("{}:", f.label), options.to_vec())
            .with_help_message(f.why)
            .with_starting_cursor(default)
            .prompt()?;
        Ok(options.iter().position(|o| *o == choice).unwrap_or(default))
    }
}

/// Backend login check: config, kind, env overlay.
pub type ValidateBackend<'a> =
    dyn Fn(&Config, BackendKind, &dyn Fn(&str) -> Option<String>) -> Result<(), BoxError> + 'a;

/// Side effects the wizard needs, injectable for tests.
pub struct Deps<'a> {
    /// Log in to the backend with this config and env overlay.
    pub validate_backend: &'a ValidateBackend<'a>,
    /// `GET /user`; returns the login.
    pub validate_github: &'a dyn Fn(&str) -> Result<String, BoxError>,
    /// Token already available (env, stored, gh) and its label.
    pub existing_github: &'a dyn Fn() -> Option<(String, String)>,
    pub env: &'a dyn Fn(&str) -> Option<String>,
    pub home: &'a Path,
}

/// What the wizard will write. Secrets are held, never displayed.
#[derive(Default)]
pub struct Plan {
    pub config_path: PathBuf,
    /// New config.toml text, when the backend step ran.
    pub config_text: Option<String>,
    pub secret_file: Option<(PathBuf, String)>,
    pub github_token: Option<String>,
    /// Lines for the review screen (no secret values).
    pub review: Vec<String>,
}

const MAX_TRIES: usize = 3;
const DEFAULT_SITE: &str = "https://app.infisical.com";

fn q(s: &str) -> String {
    toml::Value::String(s.to_string()).to_string()
}

/// config.toml for a fresh setup. Paths use `~` so the file is portable.
pub fn render_config(kind: BackendKind, a: &Answers) -> String {
    let mut s = String::from("# Written by 'diegops init'. See 'diegops explain backend'.\n");
    match kind {
        BackendKind::Infisical => {
            s.push_str("backend = \"infisical\"\n\n[infisical]\n");
            if a.site_url != DEFAULT_SITE {
                s.push_str(&format!("site_url = {}\n", q(&a.site_url)));
            }
            s.push_str(&format!("project_id = {}\n", q(&a.project_id)));
            s.push_str(&format!("client_id = {}\n", q(&a.client_id)));
            s.push_str(&format!(
                "client_secret_file = {}\n",
                q(&a.client_secret_file)
            ));
        }
        BackendKind::Vault => {
            s.push_str("backend = \"vault\"\n\n[vault]\n");
            s.push_str(&format!("address = {}\n", q(&a.vault_addr)));
        }
    }
    s
}

/// Answers collected for the backend step.
#[derive(Debug, Default, Clone)]
pub struct Answers {
    pub site_url: String,
    pub project_id: String,
    pub client_id: String,
    /// As written in config.toml (may start with `~`).
    pub client_secret_file: String,
    pub vault_addr: String,
}

fn ask_text(
    p: &mut dyn Prompter,
    given: Option<&String>,
    key: &str,
    default: Option<&str>,
) -> Result<String, BoxError> {
    if let Some(v) = given.filter(|v| !v.trim().is_empty()) {
        return Ok(v.trim().to_string());
    }
    let f = field(key);
    let v = p.text(f, default)?;
    if v.is_empty() {
        return Err(missing(f));
    }
    Ok(v)
}

/// Runs the steps and returns the plan; writes nothing.
pub fn plan(
    given: &Given,
    existing: Option<&Config>,
    config_path: &Path,
    p: &mut dyn Prompter,
    interactive: bool,
    d: &Deps,
) -> Result<Plan, BoxError> {
    let mut out = Plan {
        config_path: config_path.to_path_buf(),
        ..Default::default()
    };
    let reset = |s: &str| given.reset.iter().any(|r| r == s || r == "all");

    // Backend + credentials: skipped when the existing config already logs in.
    let current_ok = existing.and_then(|cfg| {
        let kind = config::select_kind(cfg, given.backend.as_deref(), d.env).ok()?;
        (d.validate_backend)(cfg, kind, d.env).ok().map(|_| kind)
    });
    match current_ok {
        Some(kind) if !reset("backend") => out.review.push(format!(
            "✓ backend       {} (config.toml works; --reset backend to redo)",
            kind_name(kind)
        )),
        _ => backend_step(given, existing, p, interactive, d, &mut out)?,
    }

    // GitHub token.
    if given.skip_github {
        out.review
            .push("- github token  skipped (--skip-github)".into());
    } else if let Some((tok, label)) = (d.existing_github)().filter(|_| !reset("github")) {
        let login = (d.validate_github)(&tok)
            .map_err(|e| format!("GitHub token from {label} was rejected: {e}\n  hint: fix or unset it, or run 'diegops auth gh login'"))?;
        out.review
            .push(format!("✓ github token  {login} via {label}"));
    } else {
        let f = Field {
            key: "github",
            label: "GitHub token (PAT, repo scope)",
            flag: "skip-github",
            env: "GITHUB_TOKEN",
            why: "sync and private repos; see 'diegops explain pat'",
        };
        let login = retry(interactive, || {
            let tok = p.secret(&f)?;
            let login = (d.validate_github)(&tok)?;
            Ok((tok, login))
        })?;
        out.review.push(format!(
            "+ github token  {} → ~/.diegops/tokens/gh.json (mode 600)",
            login.1
        ));
        out.github_token = Some(login.0);
    }
    Ok(out)
}

fn kind_name(k: BackendKind) -> &'static str {
    match k {
        BackendKind::Infisical => "infisical",
        BackendKind::Vault => "vault",
    }
}

/// Up to [`MAX_TRIES`] attempts when interactive; one otherwise.
fn retry<T>(interactive: bool, mut f: impl FnMut() -> Result<T, BoxError>) -> Result<T, BoxError> {
    let tries = if interactive { MAX_TRIES } else { 1 };
    let mut last = None;
    for i in 0..tries {
        match f() {
            Ok(v) => return Ok(v),
            Err(e) => {
                if i + 1 < tries {
                    eprintln!("  ✗ {e} (try {}/{tries})", i + 1);
                }
                last = Some(e);
            }
        }
    }
    Err(last.expect("at least one try"))
}

fn backend_step(
    given: &Given,
    existing: Option<&Config>,
    p: &mut dyn Prompter,
    interactive: bool,
    d: &Deps,
    out: &mut Plan,
) -> Result<(), BoxError> {
    let options = ["infisical", "vault"];
    let kind = match given.backend.as_deref() {
        Some(b) => config::select_kind(&Config::default(), Some(b), &|_| None)?,
        None => {
            if p.select(field("backend"), &options, 0)? == 1 {
                BackendKind::Vault
            } else {
                BackendKind::Infisical
            }
        }
    };
    let mut a = Answers::default();
    match kind {
        BackendKind::Infisical => {
            // Defaults come from the current config, so `--reset backend` only
            // asks for what changes.
            let old = existing.map(|c| &c.infisical);
            let site = old
                .and_then(|c| c.site_url.as_deref())
                .unwrap_or(DEFAULT_SITE);
            a.site_url = ask_text(p, given.site_url.as_ref(), "site_url", Some(site))?;
            let pid = old.and_then(|c| c.project_id.as_deref());
            a.project_id = ask_text(p, given.project_id.as_ref(), "project_id", pid)?;
            let cid = old.and_then(|c| c.client_id.as_deref());
            a.client_id = ask_text(p, given.client_id.as_ref(), "client_id", cid)?;
            let default_file = "~/.config/diegops/infisical-client-secret";
            let (file_cfg, secret) = match &given.client_secret_file {
                Some(f) => (f.display().to_string(), None),
                None => (default_file.to_string(), given.client_secret.clone()),
            };
            a.client_secret_file = file_cfg;
            let mut secret_holder = secret;
            let given_file = given.client_secret_file.is_some();
            let validated = retry(
                interactive && !given_file && given.client_secret.is_none(),
                || {
                    let s = match (&secret_holder, given_file) {
                        (_, true) => None,
                        (Some(s), _) => Some(s.clone()),
                        (None, _) => Some(p.secret(field("client_secret"))?),
                    };
                    let cfg = config::parse(&render_config(kind, &a))?;
                    let overlay = |k: &str| match (k, &s) {
                        ("INFISICAL_UNIVERSAL_AUTH_CLIENT_SECRET", Some(v)) => Some(v.clone()),
                        // The wizard's answers win over a stale exported token.
                        ("INFISICAL_TOKEN", _) => None,
                        _ => (d.env)(k),
                    };
                    (d.validate_backend)(&cfg, kind, &overlay)?;
                    secret_holder = None;
                    Ok(s)
                },
            )?;
            if let Some(s) = validated {
                let path = config::expand(&a.client_secret_file, d.home);
                out.review
                    .push(format!("+ client secret {} (mode 600)", path.display()));
                out.secret_file = Some((path, s));
            }
        }
        BackendKind::Vault => {
            let old = existing.and_then(|c| c.vault.address.as_deref());
            a.vault_addr = ask_text(p, given.vault_addr.as_ref(), "vault_addr", old)?;
            let cfg = config::parse(&render_config(kind, &a))?;
            (d.validate_backend)(&cfg, kind, d.env).map_err(|e| {
                format!(
                    "{e}\n  hint: log in first ('bao login -method=oidc') or export VAULT_TOKEN"
                )
            })?;
        }
    }
    out.review.push(format!(
        "{} config.toml   {} (backend {}, credentials accepted)",
        if existing.is_some() { "~" } else { "+" },
        out.config_path.display(),
        kind_name(kind)
    ));
    out.config_text = Some(render_config(kind, &a));
    Ok(())
}

/// Writes the plan: secret file and config (0600), GitHub token, with a
/// `.bak` of a replaced config.
pub fn apply(plan: &Plan) -> Result<(), BoxError> {
    if let Some((path, secret)) = &plan.secret_file {
        write_private(path, &format!("{secret}\n"))?;
    }
    if let Some(text) = &plan.config_text {
        if plan.config_path.exists() {
            let bak = plan.config_path.with_extension("toml.bak");
            std::fs::copy(&plan.config_path, &bak)?;
            eprintln!("  backup: {}", bak.display());
        }
        write_private(&plan.config_path, text)?;
    }
    if let Some(tok) = &plan.github_token {
        super::auth::save_gh_token(tok)?;
    }
    Ok(())
}

fn write_private(path: &Path, text: &str) -> Result<(), BoxError> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, text)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

/// `diegops init`.
pub fn run(given: Given) -> Result<(), BoxError> {
    use std::io::IsTerminal;
    let f = crate::ui::flags();
    let env = crate::secrets_backend::real_env;
    let home = crate::secrets_backend::home();
    let interactive = crate::ui::can_prompt(
        std::io::stdin().is_terminal(),
        std::io::stderr().is_terminal(),
        f.no_input,
        &env,
    );
    let path = config::default_path(&env, &home);
    let existing = if path.exists() {
        Some(config::load(&path)?)
    } else {
        None
    };
    let validate_backend = |cfg: &Config,
                            kind: BackendKind,
                            env: &dyn Fn(&str) -> Option<String>|
     -> Result<(), BoxError> {
        crate::secrets_backend::open_with_env(cfg, kind, env, &home)?.ping()
    };
    let validate_github = |t: &str| super::auth::gh_user(t);
    let existing_github = || {
        super::auth::load_gh_token_with_source()
            .ok()
            .flatten()
            .map(|(t, s)| (t, s.label()))
    };
    let deps = Deps {
        validate_backend: &validate_backend,
        validate_github: &validate_github,
        existing_github: &existing_github,
        env: &env,
        home: &home,
    };
    crate::progress!("diegops init: set up this workstation (Ctrl-C to stop; nothing is written before the review)\n");
    let plan = if interactive {
        plan(&given, existing.as_ref(), &path, &mut Inquire, true, &deps)?
    } else {
        plan(
            &given,
            existing.as_ref(),
            &path,
            &mut NoPrompt,
            false,
            &deps,
        )?
    };
    eprintln!("\nReview:");
    for l in &plan.review {
        eprintln!("  {l}");
    }
    let writes =
        plan.config_text.is_some() || plan.secret_file.is_some() || plan.github_token.is_some();
    if f.dry_run {
        eprintln!("\n(dry run: nothing written)");
        return Ok(());
    }
    if writes {
        if !crate::ui::confirm("Write these changes?")? {
            eprintln!("Nothing written.");
            return Ok(());
        }
        apply(&plan)?;
    } else {
        eprintln!("\nNothing to change.");
    }
    eprintln!();
    super::doctor::run(false, false)?;
    eprintln!("\nNext: 'diegops bootstrap' pulls repos, .env files and secret files.");
    Ok(())
}

#[cfg(test)]
#[path = "init_tests.rs"]
mod tests;
