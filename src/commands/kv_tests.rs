use super::*;

#[test]
fn put_arg_reads_value_from_file() {
    let f = std::env::temp_dir().join(format!("diegops-kv-put-{}", std::process::id()));
    std::fs::write(&f, "multi\nline\n").unwrap();
    let (k, v) = parse_put_arg(&format!("KEY=@{}", f.display()), &mut "".as_bytes()).unwrap();
    assert_eq!(k, "KEY");
    assert_eq!(v, "multi\nline\n");
    std::fs::remove_file(&f).unwrap();
}

#[test]
fn put_arg_reads_value_from_stdin() {
    let (k, v) = parse_put_arg("TOKEN=-", &mut "s3cr3t".as_bytes()).unwrap();
    assert_eq!((k.as_str(), v.as_str()), ("TOKEN", "s3cr3t"));
}

#[test]
fn put_arg_rejects_inline_values_to_keep_them_out_of_shell_history() {
    assert!(parse_put_arg("TOKEN=plain", &mut "".as_bytes()).is_err());
    assert!(parse_put_arg("=@x", &mut "".as_bytes()).is_err());
    assert!(parse_put_arg("NOEQUALS", &mut "".as_bytes()).is_err());
}

fn cfg_fixture() -> config::Config {
    toml::from_str(
        r#"
backend = "vault"
[infisical]
project_id = "proj-1"
environment = "prod"
[[infisical.mounts]]
mount = "cadibrains"
project_id = "proj-2"
[vault]
address = "https://bao.example:8200"
namespace = "personal"
"#,
    )
    .unwrap()
}

fn no_env(_: &str) -> Option<String> {
    None
}

#[test]
fn list_root_path_gives_clear_message_with_configured_mounts() {
    let msg = root_list_message(&cfg_fixture());
    assert!(msg.contains("mount"), "{msg}");
    assert!(msg.contains("cadibrains"), "{msg}");
    assert!(!msg.contains("empty secret path"));
    assert!(is_root_path("/"));
    assert!(is_root_path(""));
    assert!(is_root_path(" // "));
    assert!(!is_root_path("dpinto-work"));
}

#[test]
fn migrate_header_shows_source_address_namespace_and_target_project_env() {
    let h = migrate_header(
        &cfg_fixture(),
        config::BackendKind::Vault,
        config::BackendKind::Infisical,
        &no_env,
    );
    assert!(h.contains("https://bao.example:8200"), "{h}");
    assert!(h.contains("namespace personal"), "{h}");
    assert!(h.contains("proj-1"), "{h}");
    assert!(h.contains("env prod"), "{h}");
    assert!(h.contains("cadibrains") && h.contains("proj-2"), "{h}");
    assert!(!h.to_lowercase().contains("warning"), "{h}");
}

#[test]
fn migrate_header_warns_when_vault_addr_env_overrides_config() {
    let env = |k: &str| (k == "VAULT_ADDR").then(|| "https://other.example".to_string());
    let h = migrate_header(
        &cfg_fixture(),
        config::BackendKind::Vault,
        config::BackendKind::Infisical,
        &env,
    );
    assert!(h.contains("https://other.example"), "{h}");
    assert!(h.contains("warning") && h.contains("VAULT_ADDR"), "{h}");
}

#[test]
fn migrate_header_never_includes_tokens() {
    let env = |k: &str| (k == "VAULT_TOKEN").then(|| "hvs.SECRET".to_string());
    let h = migrate_header(
        &cfg_fixture(),
        config::BackendKind::Vault,
        config::BackendKind::Infisical,
        &env,
    );
    assert!(!h.contains("hvs.SECRET"));
}
