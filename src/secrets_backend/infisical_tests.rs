use super::*;
use crate::secrets_backend::config::{
    InfisicalAuth, InfisicalConfig, InfisicalSettings, Mapping, MountMapping,
};
use mockito::Matcher;

fn settings(url: &str, auth: InfisicalAuth) -> InfisicalSettings {
    let cfg = InfisicalConfig {
        project_id: Some("proj".into()),
        environment: Some("prod".into()),
        mounts: vec![MountMapping {
            mount: "dpinto-work".into(),
            project_id: Some("proj-work".into()),
            environment: None,
            folder: Some("/".into()),
        }],
        ..Default::default()
    };
    InfisicalSettings {
        site_url: url.to_string(),
        auth,
        mapping: Mapping::from_config(&cfg),
    }
}

fn backend(server: &mockito::Server) -> InfisicalBackend {
    InfisicalBackend::new(settings(
        &server.url(),
        InfisicalAuth::AccessToken("tok".into()),
    ))
    .with_retry_delay_ms(1)
}

fn list_query(path: &str) -> Matcher {
    Matcher::AllOf(vec![
        Matcher::UrlEncoded("projectId".into(), "proj".into()),
        Matcher::UrlEncoded("environment".into(), "prod".into()),
        Matcher::UrlEncoded("secretPath".into(), path.into()),
        Matcher::UrlEncoded("viewSecretValue".into(), "true".into()),
        Matcher::UrlEncoded("expandSecretReferences".into(), "false".into()),
    ])
}

#[test]
fn universal_auth_logs_in_once_and_caches_token() {
    let mut server = mockito::Server::new();
    let login = server
        .mock("POST", "/api/v1/auth/universal-auth/login")
        .match_body(Matcher::Json(serde_json::json!({"clientId":"cid","clientSecret":"cs"})))
        .with_body(r#"{"accessToken":"at-1","expiresIn":7200,"accessTokenMaxTTL":7200,"tokenType":"Bearer"}"#)
        .expect(1)
        .create();
    let get = server
        .mock("GET", "/api/v4/secrets")
        .match_query(Matcher::Any)
        .match_header("authorization", "Bearer at-1")
        .with_body(r#"{"secrets":[],"imports":[]}"#)
        .expect(2)
        .create();
    let b = InfisicalBackend::new(settings(
        &server.url(),
        InfisicalAuth::UniversalAuth {
            client_id: "cid".into(),
            client_secret: "cs".into(),
        },
    ));
    b.get("dpinto-config/a").unwrap();
    b.get("dpinto-config/b").unwrap();
    login.assert();
    get.assert();
}

#[test]
fn get_maps_path_to_folder_and_decodes_values() {
    let mut server = mockito::Server::new();
    let enc = codec::encode("line1\nline2\n");
    let body = serde_json::json!({"secrets":[
        {"secretKey":"API_KEY","secretValue":"abc"},
        {"secretKey":"id_ed25519","secretValue":enc}
    ],"imports":[]});
    let m = server
        .mock("GET", "/api/v4/secrets")
        .match_query(list_query("/dpinto-config/app/env"))
        .match_header("authorization", "Bearer tok")
        .with_body(body.to_string())
        .create();
    let got = backend(&server)
        .get("dpinto-config/app/env")
        .unwrap()
        .unwrap();
    m.assert();
    assert_eq!(got.get("API_KEY").map(String::as_str), Some("abc"));
    assert_eq!(
        got.get("id_ed25519").map(String::as_str),
        Some("line1\nline2\n")
    );
}

#[test]
fn get_missing_folder_is_none() {
    let mut server = mockito::Server::new();
    server
        .mock("GET", "/api/v4/secrets")
        .match_query(Matcher::Any)
        .with_status(404)
        .with_body(r#"{"statusCode":404,"message":"Folder not found"}"#)
        .create();
    assert!(backend(&server)
        .get("dpinto-config/nope")
        .unwrap()
        .is_none());
}

#[test]
fn get_empty_folder_is_none() {
    let mut server = mockito::Server::new();
    server
        .mock("GET", "/api/v4/secrets")
        .match_query(Matcher::Any)
        .with_body(r#"{"secrets":[],"imports":[]}"#)
        .create();
    assert!(backend(&server)
        .get("dpinto-config/empty")
        .unwrap()
        .is_none());
}

#[test]
fn rate_limited_requests_are_retried() {
    let mut server = mockito::Server::new();
    let limited = server
        .mock("GET", "/api/v4/secrets")
        .match_query(Matcher::Any)
        .with_status(429)
        .expect(1)
        .create();
    let ok = server
        .mock("GET", "/api/v4/secrets")
        .match_query(Matcher::Any)
        .with_body(r#"{"secrets":[{"secretKey":"K","secretValue":"v"}]}"#)
        .expect(1)
        .create();
    let got = backend(&server).get("dpinto-config/x").unwrap().unwrap();
    limited.assert();
    ok.assert();
    assert_eq!(got.len(), 1);
}

#[test]
fn put_creates_folder_and_upserts_only_changed_keys() {
    let mut server = mockito::Server::new();
    let folder = server
        .mock("POST", "/api/v2/folders")
        .match_body(Matcher::Json(serde_json::json!({
            "projectId":"proj","environment":"prod","name":"env","path":"/dpinto-config/app"
        })))
        .with_status(400)
        .with_body(r#"{"statusCode":400,"message":"Folder with name 'env' already exists in path '/dpinto-config/app'"}"#)
        .expect(1)
        .create();
    server
        .mock("GET", "/api/v4/secrets")
        .match_query(list_query("/dpinto-config/app/env"))
        .with_body(r#"{"secrets":[{"secretKey":"SAME","secretValue":"1"},{"secretKey":"OLD","secretValue":"x"}]}"#)
        .create();
    let patch = server
        .mock("PATCH", "/api/v4/secrets/batch")
        .match_body(Matcher::Json(serde_json::json!({
            "projectId":"proj","environment":"prod","secretPath":"/dpinto-config/app/env",
            "mode":"upsert",
            "secrets":[
                {"secretKey":"NEW","secretValue":codec::encode("a\nb")},
                {"secretKey":"OLD","secretValue":"y"}
            ]
        })))
        .with_body(r#"{"secrets":[]}"#)
        .expect(1)
        .create();
    let mut data = SecretMap::new();
    data.insert("SAME".into(), "1".into());
    data.insert("OLD".into(), "y".into());
    data.insert("NEW".into(), "a\nb".into());
    backend(&server)
        .put("dpinto-config/app/env", &data)
        .unwrap();
    folder.assert();
    patch.assert();
}

#[test]
fn put_unchanged_makes_no_write() {
    let mut server = mockito::Server::new();
    server
        .mock("POST", "/api/v2/folders")
        .with_body("{}")
        .create();
    server
        .mock("GET", "/api/v4/secrets")
        .match_query(Matcher::Any)
        .with_body(r#"{"secrets":[{"secretKey":"K","secretValue":"v"}]}"#)
        .create();
    let patch = server
        .mock("PATCH", "/api/v4/secrets/batch")
        .expect(0)
        .create();
    let mut data = SecretMap::new();
    data.insert("K".into(), "v".into());
    backend(&server).put("dpinto-config/x", &data).unwrap();
    patch.assert();
}

#[test]
fn put_error_never_echoes_secret_values() {
    let mut server = mockito::Server::new();
    server
        .mock("POST", "/api/v2/folders")
        .with_body("{}")
        .create();
    server
        .mock("GET", "/api/v4/secrets")
        .match_query(Matcher::Any)
        .with_status(404)
        .create();
    server
        .mock("PATCH", "/api/v4/secrets/batch")
        .with_status(400)
        .with_body(r#"{"message":"bad value TOPSECRETVALUE"}"#)
        .create();
    let mut data = SecretMap::new();
    data.insert("K".into(), "TOPSECRETVALUE".into());
    let err = backend(&server)
        .put("dpinto-config/x", &data)
        .err()
        .unwrap()
        .to_string();
    assert!(err.contains("400"), "{err}");
    assert!(!err.contains("TOPSECRETVALUE"), "{err}");
}

#[test]
fn list_returns_child_folders() {
    let mut server = mockito::Server::new();
    server
        .mock("GET", "/api/v2/folders")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("projectId".into(), "proj".into()),
            Matcher::UrlEncoded("environment".into(), "prod".into()),
            Matcher::UrlEncoded("path".into(), "/dpinto-config".into()),
        ]))
        .with_body(r#"{"folders":[{"id":"1","name":"app"},{"id":"2","name":"cli"}]}"#)
        .create();
    let got = backend(&server).list("dpinto-config").unwrap();
    assert_eq!(got, vec!["app/".to_string(), "cli/".to_string()]);
}

#[test]
fn location_id_identifies_project_env_and_folder() {
    let server = mockito::Server::new();
    let b = backend(&server);
    assert_eq!(
        b.location_id("dpinto-work/gpg").unwrap(),
        "proj-work/prod:/gpg"
    );
}
