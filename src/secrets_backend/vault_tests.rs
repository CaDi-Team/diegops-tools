use super::*;
use crate::secrets_backend::config::VaultSettings;
use mockito::Matcher;

fn backend(server: &mockito::Server) -> VaultBackend {
    VaultBackend::new(VaultSettings {
        address: server.url(),
        namespace: Some("personal".into()),
        ca_cert: None,
        token: "vt".into(),
    })
    .unwrap()
}

#[test]
fn get_reads_kv_v2_with_token_and_namespace_headers() {
    let mut server = mockito::Server::new();
    let m = server
        .mock("GET", "/v1/dpinto-config/data/app/env")
        .match_header("x-vault-token", "vt")
        .match_header("x-vault-namespace", "personal")
        .with_body(r#"{"data":{"data":{"A":"1","N":5},"metadata":{}}}"#)
        .create();
    let got = backend(&server).get("dpinto-config/app/env").unwrap().unwrap();
    m.assert();
    assert_eq!(got.get("A").map(String::as_str), Some("1"));
    assert_eq!(got.get("N").map(String::as_str), Some("5"));
}

#[test]
fn get_missing_is_none() {
    let mut server = mockito::Server::new();
    server.mock("GET", "/v1/dpinto-config/data/nope").with_status(404).with_body(r#"{"errors":[]}"#).create();
    assert!(backend(&server).get("dpinto-config/nope").unwrap().is_none());
}

#[test]
fn get_requires_mount_and_path() {
    let server = mockito::Server::new();
    assert!(backend(&server).get("onlymount").is_err());
}

#[test]
fn list_uses_list_method_on_metadata() {
    let mut server = mockito::Server::new();
    server
        .mock("LIST", "/v1/dpinto-work/metadata/keys")
        .with_body(r#"{"data":{"keys":["gpg","ssh/"]}}"#)
        .create();
    let got = backend(&server).list("dpinto-work/keys").unwrap();
    assert_eq!(got, vec!["gpg".to_string(), "ssh/".to_string()]);
}

#[test]
fn list_mount_root() {
    let mut server = mockito::Server::new();
    server
        .mock("LIST", "/v1/dpinto-work/metadata/")
        .with_body(r#"{"data":{"keys":["a"]}}"#)
        .create();
    assert_eq!(backend(&server).list("dpinto-work").unwrap(), vec!["a".to_string()]);
}

#[test]
fn put_merges_with_existing_keys() {
    let mut server = mockito::Server::new();
    server
        .mock("GET", "/v1/m/data/p")
        .with_body(r#"{"data":{"data":{"KEEP":"k","X":"old"}}}"#)
        .create();
    let w = server
        .mock("POST", "/v1/m/data/p")
        .match_body(Matcher::Json(serde_json::json!({"data":{"KEEP":"k","X":"new"}})))
        .with_body("{}")
        .expect(1)
        .create();
    let mut data = SecretMap::new();
    data.insert("X".into(), "new".into());
    backend(&server).put("m/p", &data).unwrap();
    w.assert();
}

#[test]
fn forbidden_is_reported_without_body() {
    let mut server = mockito::Server::new();
    server.mock("GET", "/v1/m/data/p").with_status(403).with_body(r#"{"errors":["permission denied"]}"#).create();
    let err = backend(&server).get("m/p").err().unwrap().to_string();
    assert!(err.contains("403"), "{err}");
}
