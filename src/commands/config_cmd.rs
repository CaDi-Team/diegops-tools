//! `diegops config path`: every file diegops reads and the rule that chose it
//! (ADR 0001 D7). config.toml = how diegops connects; ~/.diegops/*.yaml =
//! what it manages.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::secrets_backend::config;

type BoxError = Box<dyn std::error::Error>;

/// One resolved path.
#[derive(Debug, Serialize, PartialEq)]
pub struct Entry {
    pub name: &'static str,
    pub path: PathBuf,
    /// `$VAR`, `default` or `config.toml [section] key`.
    pub rule: String,
    pub exists: bool,
    pub purpose: &'static str,
}

/// YAML files under ~/.diegops: (file, env override, purpose).
pub const YAMLS: &[(&str, &str, &str)] = &[
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

fn set(env: &dyn Fn(&str) -> Option<String>, k: &str) -> Option<String> {
    env(k).filter(|v| !v.is_empty())
}

/// Resolves every path without touching the filesystem except `exists`.
pub fn entries(
    env: &dyn Fn(&str) -> Option<String>,
    home: &Path,
    exists: &dyn Fn(&Path) -> bool,
) -> Vec<Entry> {
    let mut out = Vec::new();
    let mut push = |name, path: PathBuf, rule: String, purpose| {
        let e = exists(&path);
        out.push(Entry {
            name,
            path,
            rule,
            exists: e,
            purpose,
        });
    };
    let cfg_path = config::default_path(env, home);
    let rule = if set(env, "DIEGOPS_CONFIG").is_some() {
        "$DIEGOPS_CONFIG".to_string()
    } else if set(env, "XDG_CONFIG_HOME").is_some() {
        "$XDG_CONFIG_HOME/diegops/config.toml".to_string()
    } else {
        "default".to_string()
    };
    push(
        "config.toml",
        cfg_path.clone(),
        rule,
        "how diegops connects: backend and credential refs",
    );
    if let Ok(cfg) = config::load(&cfg_path) {
        if let Some(f) = cfg.infisical.client_secret_file.as_deref() {
            push(
                "client secret",
                config::expand(f, home),
                "config.toml [infisical] client_secret_file".into(),
                "Infisical Client Secret (mode 600)",
            );
        }
    }
    let dir = home.join(".diegops");
    for (file, var, purpose) in YAMLS {
        let (path, rule) = match set(env, var) {
            Some(p) => (PathBuf::from(p), format!("${var}")),
            None => (dir.join(file), "default".to_string()),
        };
        push(file, path, rule, purpose);
    }
    push(
        "tokens",
        dir.join("tokens"),
        "default".into(),
        "stored GitHub token (gh.json, mode 600)",
    );
    out
}

/// `diegops config path`.
pub fn path(json: bool) -> Result<(), BoxError> {
    let env = crate::secrets_backend::real_env;
    let home = crate::secrets_backend::home();
    let list = entries(&env, &home, &|p: &Path| p.exists());
    if json {
        return crate::ui::print_json(&serde_json::json!({ "paths": list }));
    }
    for e in &list {
        println!(
            "{:<15} {}{}",
            e.name,
            e.path.display(),
            if e.exists { "" } else { "  (missing)" }
        );
        println!("{:<15} ↳ {} · {}", "", e.rule, e.purpose);
    }
    println!("\nconfig.toml says how diegops connects; ~/.diegops/*.yaml say what it manages");
    println!("('diegops sync' backs those up). See 'diegops explain config'.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env_of(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let m: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |k| m.get(k).cloned()
    }

    #[test]
    fn defaults_live_in_config_and_dot_diegops() {
        let home = Path::new("/h");
        let list = entries(&env_of(&[]), home, &|_| false);
        let get = |n: &str| list.iter().find(|e| e.name == n).unwrap();
        assert_eq!(
            get("config.toml").path,
            Path::new("/h/.config/diegops/config.toml")
        );
        assert_eq!(get("config.toml").rule, "default");
        assert_eq!(get("repos.yaml").path, Path::new("/h/.diegops/repos.yaml"));
        assert_eq!(get("tokens").path, Path::new("/h/.diegops/tokens"));
        assert!(!get("agents.yaml").exists);
    }

    #[test]
    fn env_overrides_are_named_as_the_rule() {
        let env = env_of(&[
            ("DIEGOPS_CONFIG", "/c.toml"),
            ("DIEGOPS_SECRETS_CONFIG", "/s.yaml"),
        ]);
        let list = entries(&env, Path::new("/h"), &|p| p == Path::new("/s.yaml"));
        let get = |n: &str| list.iter().find(|e| e.name == n).unwrap();
        assert_eq!(get("config.toml").rule, "$DIEGOPS_CONFIG");
        assert_eq!(get("secrets.yaml").rule, "$DIEGOPS_SECRETS_CONFIG");
        assert!(get("secrets.yaml").exists);
        let env = env_of(&[("XDG_CONFIG_HOME", "/x")]);
        let list = entries(&env, Path::new("/h"), &|_| false);
        assert_eq!(list[0].path, Path::new("/x/diegops/config.toml"));
        assert_eq!(list[0].rule, "$XDG_CONFIG_HOME/diegops/config.toml");
    }

    #[test]
    fn doctor_and_config_path_agree_on_the_yaml_files() {
        let names: Vec<_> = YAMLS.iter().map(|(f, v, _)| (*f, *v)).collect();
        let doctor: Vec<_> = super::super::doctor::YAML_CONFIGS
            .iter()
            .map(|(f, v, _)| (*f, *v))
            .collect();
        assert_eq!(names, doctor);
    }
}
