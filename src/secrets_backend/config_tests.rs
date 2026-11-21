use super::*;
use std::collections::HashMap;

fn env_of(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
    let m: HashMap<String, String> = pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    move |k| m.get(k).cloned()
}

const SAMPLE: &str = r#"
backend = "infisical"

[infisical]
project_id = "proj-default"
environment = "prod"
client_id = "cid"

[[infisical.mounts]]
mount = "dpinto-work"
project_id = "proj-work"
environment = "prod"
folder = "/work"

[vault]
address = "https://bao.example.test"
namespace = "personal"
"#;

#[test]
fn parses_full_config() {
    let cfg = parse(SAMPLE).unwrap();
    assert_eq!(cfg.backend.as_deref(), Some("infisical"));
    assert_eq!(cfg.infisical.mounts.len(), 1);
    assert_eq!(cfg.vault.namespace.as_deref(), Some("personal"));
}

#[test]
fn rejects_unknown_keys() {
    assert!(parse("bakend = \"vault\"").is_err());
}

#[test]
fn missing_config_file_yields_defaults() {
    let cfg = load(Path::new("/nonexistent/diegops/config.toml")).unwrap();
    assert!(cfg.backend.is_none());
}

#[test]
fn default_path_honours_env_then_xdg_then_home() {
    let home = Path::new("/home/u");
    assert_eq!(
        default_path(&env_of(&[("DIEGOPS_CONFIG", "/x/c.toml")]), home),
        PathBuf::from("/x/c.toml")
    );
    assert_eq!(
        default_path(&env_of(&[("XDG_CONFIG_HOME", "/xdg")]), home),
        PathBuf::from("/xdg/diegops/config.toml")
    );
    assert_eq!(
        default_path(&env_of(&[]), home),
        PathBuf::from("/home/u/.config/diegops/config.toml")
    );
}

#[test]
fn infisical_is_the_default_backend() {
    let cfg = Config::default();
    assert_eq!(
        select_kind(&cfg, None, &env_of(&[])).unwrap(),
        BackendKind::Infisical
    );
}

#[test]
fn backend_precedence_flag_then_env_then_file() {
    let cfg = parse("backend = \"infisical\"").unwrap();
    let env = env_of(&[("DIEGOPS_SECRETS_BACKEND", "vault")]);
    assert_eq!(select_kind(&cfg, None, &env).unwrap(), BackendKind::Vault);
    assert_eq!(
        select_kind(&cfg, Some("infisical"), &env).unwrap(),
        BackendKind::Infisical
    );
    assert!(select_kind(&cfg, Some("consul"), &env).is_err());
}

#[test]
fn vault_has_no_hard_coded_address() {
    let cfg = Config::default();
    let err = resolve_vault(&cfg, &env_of(&[("VAULT_TOKEN", "t")]), Path::new("/nohome"))
        .err()
        .unwrap()
        .to_string();
    assert!(err.contains("address"), "{err}");
}

#[test]
fn vault_env_overrides_file() {
    let cfg = parse(SAMPLE).unwrap();
    let env = env_of(&[
        ("VAULT_ADDR", "https://other.test"),
        ("VAULT_NAMESPACE", "ns2"),
        ("VAULT_TOKEN", "tok"),
        ("VAULT_CACERT", "/ca.pem"),
    ]);
    let v = resolve_vault(&cfg, &env, Path::new("/nohome")).unwrap();
    assert_eq!(v.address, "https://other.test");
    assert_eq!(v.namespace.as_deref(), Some("ns2"));
    assert_eq!(v.ca_cert, Some(PathBuf::from("/ca.pem")));
    assert_eq!(v.token, "tok");
}

#[test]
fn vault_token_falls_back_to_vault_token_file() {
    let dir = std::env::temp_dir().join(format!("diegops-cfg-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(".vault-token"), "from-file\n").unwrap();
    let cfg = parse(SAMPLE).unwrap();
    let v = resolve_vault(&cfg, &env_of(&[]), &dir).unwrap();
    assert_eq!(v.token, "from-file");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn infisical_defaults_to_cloud_and_universal_auth_from_env() {
    let cfg = parse(SAMPLE).unwrap();
    let env = env_of(&[("INFISICAL_UNIVERSAL_AUTH_CLIENT_SECRET", "csecret")]);
    let s = resolve_infisical(&cfg, &env).unwrap();
    assert_eq!(s.site_url, "https://app.infisical.com");
    match s.auth {
        InfisicalAuth::UniversalAuth {
            client_id,
            client_secret,
        } => {
            assert_eq!(client_id, "cid");
            assert_eq!(client_secret, "csecret");
        }
        InfisicalAuth::AccessToken(_) => panic!("expected universal auth"),
    }
}

#[test]
fn infisical_access_token_env_wins() {
    let cfg = Config::default();
    let env = env_of(&[
        ("INFISICAL_TOKEN", "at"),
        ("INFISICAL_API_URL", "https://eu.infisical.com/"),
    ]);
    let s = resolve_infisical(&cfg, &env).unwrap();
    assert_eq!(s.site_url, "https://eu.infisical.com");
    assert!(matches!(s.auth, InfisicalAuth::AccessToken(ref t) if t == "at"));
}

#[test]
fn infisical_without_credentials_is_a_clear_error() {
    let cfg = Config::default();
    let err = resolve_infisical(&cfg, &env_of(&[]))
        .err()
        .unwrap()
        .to_string();
    assert!(
        err.contains("client_id") || err.contains("INFISICAL"),
        "{err}"
    );
}

#[cfg(unix)]
#[test]
fn client_secret_file_must_be_private() {
    use std::os::unix::fs::PermissionsExt;
    let dir = std::env::temp_dir().join(format!("diegops-sec-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("secret");
    std::fs::write(&f, "csecret\n").unwrap();
    std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o644)).unwrap();
    let toml = format!(
        "[infisical]\nproject_id='p'\nclient_id='cid'\nclient_secret_file='{}'\n",
        f.display()
    );
    let cfg = parse(&toml).unwrap();
    assert!(resolve_infisical(&cfg, &env_of(&[])).is_err());
    std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o600)).unwrap();
    let s = resolve_infisical(&cfg, &env_of(&[])).unwrap();
    assert!(
        matches!(s.auth, InfisicalAuth::UniversalAuth { ref client_secret, .. } if client_secret == "csecret")
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn mapped_mount_goes_to_its_project_and_folder() {
    let cfg = parse(SAMPLE).unwrap();
    let m = Mapping::from_config(&cfg.infisical);
    let loc = m.locate("dpinto-work/gnupg/main").unwrap();
    assert_eq!(loc.project_id, "proj-work");
    assert_eq!(loc.environment, "prod");
    assert_eq!(loc.folder, "/work/gnupg/main");
}

#[test]
fn unmapped_mount_keeps_full_path_in_default_project() {
    let cfg = parse(SAMPLE).unwrap();
    let m = Mapping::from_config(&cfg.infisical);
    let loc = m.locate("dpinto-config/karluiz-tool-cli/env").unwrap();
    assert_eq!(loc.project_id, "proj-default");
    assert_eq!(loc.folder, "/dpinto-config/karluiz-tool-cli/env");
}

#[test]
fn folder_segments_are_sanitised_for_infisical() {
    let cfg = parse(SAMPLE).unwrap();
    let m = Mapping::from_config(&cfg.infisical);
    let loc = m.locate("dpinto-config/app.android/sign ing").unwrap();
    assert_eq!(loc.folder, "/dpinto-config/app_android/sign_ing");
}

#[test]
fn locate_without_project_is_an_error() {
    let m = Mapping::from_config(&InfisicalConfig::default());
    assert!(m.locate("dpinto-config/x").is_err());
    assert!(m.locate("").is_err());
}

#[test]
fn tilde_secret_file_without_home_is_a_clear_error() {
    let cfg = InfisicalConfig {
        client_id: Some("c".into()),
        client_secret_file: Some("~/secret".into()),
        ..Default::default()
    };
    let err = client_secret(&cfg, &env_of(&[])).unwrap_err().to_string();
    assert!(err.contains("HOME is not set"), "{err}");
}
