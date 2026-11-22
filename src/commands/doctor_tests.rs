use super::*;

#[test]
fn valid_token_and_existing_sync_repo() {
    let mut s = mockito::Server::new();
    let _u = s
        .mock("GET", "/user")
        .with_body(r#"{"login":"octo"}"#)
        .create();
    let _r = s
        .mock("GET", "/repos/octo/diegops-octo-memory")
        .with_body("{}")
        .create();
    let c = github_token_check(&s.url(), "t");
    assert_eq!(c[0].status, Status::Ok);
    assert!(c[0].detail.contains("octo"));
    assert_eq!(c[1].status, Status::Ok);
}

#[test]
fn rejected_token_fails_with_hint() {
    let mut s = mockito::Server::new();
    let _u = s.mock("GET", "/user").with_status(401).create();
    let c = github_token_check(&s.url(), "t");
    assert_eq!(c.len(), 1);
    assert_eq!(c[0].status, Status::Fail);
    assert!(c[0]
        .hint
        .as_deref()
        .unwrap()
        .contains("diegops auth gh login"));
}

#[test]
fn missing_sync_repo_is_a_warning() {
    let mut s = mockito::Server::new();
    let _u = s
        .mock("GET", "/user")
        .with_body(r#"{"login":"octo"}"#)
        .create();
    let _r = s
        .mock("GET", "/repos/octo/diegops-octo-memory")
        .with_status(404)
        .create();
    let c = github_token_check(&s.url(), "t");
    assert_eq!(c[1].status, Status::Warn);
    assert!(c[1].hint.as_deref().unwrap().contains("sync push"));
}

#[cfg(unix)]
#[test]
fn secret_file_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let dir = std::env::temp_dir().join(format!("diegops-doctor-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("s");
    std::fs::write(&f, "x").unwrap();
    std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(secret_file_check(&f).status, Status::Ok);
    std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o640)).unwrap();
    let c = secret_file_check(&f);
    assert_eq!(c.status, Status::Warn);
    assert!(c.hint.unwrap().starts_with("chmod 600"));
    assert_eq!(secret_file_check(&dir.join("nope")).status, Status::Fail);
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn private_file_check_names_the_credential() {
    use std::os::unix::fs::PermissionsExt;
    let dir = std::env::temp_dir().join(format!("diegops-doctor-pf-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("gh.json");
    std::fs::write(&f, "{}").unwrap();
    std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o644)).unwrap();
    let c = private_file_check("github token file", &f);
    assert_eq!(c.name, "github token file");
    assert_eq!(c.status, Status::Warn);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn gh_token_source_labels() {
    use super::super::auth::GhTokenSource;
    assert_eq!(GhTokenSource::Env("GH_TOKEN").label(), "$GH_TOKEN");
    assert_eq!(
        GhTokenSource::Stored.label(),
        "stored (~/.diegops/tokens/gh.json)"
    );
    assert_eq!(GhTokenSource::GhCli.label(), "gh auth token");
}
