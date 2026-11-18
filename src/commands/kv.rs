//! `diegops kv` — direct access to the configured secrets backend
//! (Infisical by default, Vault/OpenBao on request). Values are never printed
//! unless a single key is asked for explicitly.

use std::io::{Read, Write};

use clap::Subcommand;

use crate::secrets_backend::{self, config, migrate, SecretMap};

type BoxError = Box<dyn std::error::Error>;

/// `kv` sub-commands.
#[derive(Subcommand)]
pub enum KvCommand {
    /// Show the selected backend and config file (no secrets)
    Backend,
    /// Show key names at a path, or one value with --key
    Get {
        /// Logical path `<mount>/<path>`, e.g. dpinto-config/kube
        path: String,
        /// Print only this key's value (raw, to stdout)
        #[arg(long)]
        key: Option<String>,
    },
    /// List sub-paths under a path
    List {
        /// Logical path, e.g. dpinto-work
        path: String,
    },
    /// Upsert keys: KEY=@file or KEY=- (stdin); inline values are refused
    Put {
        /// Logical path `<mount>/<path>`
        path: String,
        /// KEY=@file or KEY=-
        #[arg(required = true)]
        pairs: Vec<String>,
    },
    /// Copy secrets from Vault/OpenBao to Infisical (idempotent, verified by SHA-256)
    Migrate {
        /// Source prefixes (mounts or paths), e.g. dpinto-config dpinto-work
        #[arg(required = true)]
        prefixes: Vec<String>,
        /// Source backend
        #[arg(long, default_value = "vault")]
        from: String,
        /// Target backend
        #[arg(long, default_value = "infisical")]
        to: String,
        /// Show the plan without writing
        #[arg(long)]
        dry_run: bool,
    },
}

/// Runs a `kv` sub-command.
pub fn run(cmd: KvCommand, backend: Option<&str>) -> Result<(), BoxError> {
    match cmd {
        KvCommand::Backend => {
            let (path, cfg) = secrets_backend::load_config()?;
            let kind = config::select_kind(&cfg, backend, &secrets_backend::real_env)?;
            let name = match kind {
                config::BackendKind::Infisical => "infisical",
                config::BackendKind::Vault => "vault",
            };
            println!("backend: {name}");
            println!("config:  {}", path.display());
        }
        KvCommand::Get { path, key } => {
            let b = secrets_backend::open(backend)?;
            let data = b
                .get(&path)?
                .ok_or_else(|| format!("no secret at '{path}' ({})", b.name()))?;
            match key {
                Some(k) => {
                    let v = data
                        .get(&k)
                        .ok_or_else(|| format!("no key '{k}' at '{path}'"))?;
                    std::io::stdout().write_all(v.as_bytes())?;
                }
                None => {
                    for k in data.keys() {
                        println!("{k}");
                    }
                }
            }
        }
        KvCommand::List { path } => {
            let b = secrets_backend::open(backend)?;
            for e in b.list(&path)? {
                println!("{e}");
            }
        }
        KvCommand::Put { path, pairs } => {
            let b = secrets_backend::open(backend)?;
            let mut stdin = std::io::stdin();
            let mut data = SecretMap::new();
            for p in &pairs {
                let (k, v) = parse_put_arg(p, &mut stdin)?;
                data.insert(k, v);
            }
            b.put(&path, &data)?;
            println!("wrote {} key(s) to '{path}' ({})", data.len(), b.name());
        }
        KvCommand::Migrate {
            prefixes,
            from,
            to,
            dry_run,
        } => {
            let (_, cfg) = secrets_backend::load_config()?;
            let src_kind = config::select_kind(&cfg, Some(&from), &secrets_backend::real_env)?;
            let dst_kind = config::select_kind(&cfg, Some(&to), &secrets_backend::real_env)?;
            if src_kind == dst_kind {
                return Err("--from and --to must be different backends".into());
            }
            let src = secrets_backend::open_kind(&cfg, src_kind)?;
            let dst = secrets_backend::open_kind(&cfg, dst_kind)?;
            let reports = migrate::migrate(src.as_ref(), dst.as_ref(), &prefixes, dry_run)?;
            print!("{}", migrate::render(&reports));
            if dry_run {
                println!("dry run: nothing written");
            }
            if reports.iter().any(|r| r.verified == Some(false)) {
                return Err("verification failed: do NOT delete the source".into());
            }
        }
    }
    Ok(())
}

/// Parses `KEY=@file` or `KEY=-` (stdin).
pub fn parse_put_arg(arg: &str, stdin: &mut dyn Read) -> Result<(String, String), BoxError> {
    let (k, src) = arg
        .split_once('=')
        .ok_or_else(|| format!("'{arg}': expected KEY=@file or KEY=-"))?;
    if k.is_empty() {
        return Err("empty key name".into());
    }
    let value = if src == "-" {
        let mut s = String::new();
        stdin.read_to_string(&mut s)?;
        s
    } else if let Some(file) = src.strip_prefix('@') {
        std::fs::read_to_string(file).map_err(|e| format!("could not read {file}: {e}"))?
    } else {
        return Err(format!(
            "'{k}': inline values are refused (shell history); use {k}=@file or {k}=-"
        )
        .into());
    };
    Ok((k.to_string(), value))
}

#[cfg(test)]
#[path = "kv_tests.rs"]
mod tests;
