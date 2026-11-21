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
    let got = backend(&server)
        .get("dpinto-config/app/env")
        .unwrap()
        .unwrap();
    m.assert();
    assert_eq!(got.get("A").map(String::as_str), Some("1"));
    assert_eq!(got.get("N").map(String::as_str), Some("5"));
}

#[test]
fn get_missing_is_none() {
    let mut server = mockito::Server::new();
    server
        .mock("GET", "/v1/dpinto-config/data/nope")
        .with_status(404)
        .with_body(r#"{"errors":[]}"#)
        .create();
    assert!(backend(&server)
        .get("dpinto-config/nope")
        .unwrap()
        .is_none());
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
    assert_eq!(
        backend(&server).list("dpinto-work").unwrap(),
        vec!["a".to_string()]
    );
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
        .match_body(Matcher::Json(
            serde_json::json!({"data":{"KEEP":"k","X":"new"}}),
        ))
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
    server
        .mock("GET", "/v1/m/data/p")
        .with_status(403)
        .with_body(r#"{"errors":["permission denied"]}"#)
        .create();
    let err = backend(&server).get("m/p").err().unwrap().to_string();
    assert!(err.contains("403"), "{err}");
}

#[test]
fn forbidden_message_is_exact() {
    let mut server = mockito::Server::new();
    server
        .mock("LIST", "/v1/m/metadata/p")
        .with_status(403)
        .with_body(r#"{"errors":["permission denied SECRETISH"]}"#)
        .create();
    let err = backend(&server).list("m/p").err().unwrap().to_string();
    assert_eq!(err, "Vault list 'm/p' failed: HTTP 403");
}

#[test]
fn list_missing_is_empty() {
    let mut server = mockito::Server::new();
    server
        .mock("LIST", "/v1/m/metadata/none")
        .with_status(404)
        .create();
    assert!(backend(&server).list("m/none").unwrap().is_empty());
}

#[test]
fn write_error_never_echoes_body() {
    let mut server = mockito::Server::new();
    server.mock("GET", "/v1/m/data/p").with_status(404).create();
    server
        .mock("POST", "/v1/m/data/p")
        .with_status(500)
        .with_body(r#"{"errors":["TOPSECRETVALUE"]}"#)
        .create();
    let mut data = SecretMap::new();
    data.insert("X".into(), "TOPSECRETVALUE".into());
    let err = backend(&server)
        .put("m/p", &data)
        .err()
        .unwrap()
        .to_string();
    assert_eq!(err, "Vault write 'm/p' failed: HTTP 500");
}

#[test]
fn no_namespace_header_when_unset() {
    let mut server = mockito::Server::new();
    let m = server
        .mock("GET", "/v1/m/data/p")
        .match_header("x-vault-namespace", Matcher::Missing)
        .match_header(
            "user-agent",
            format!("diegops/{}", env!("CARGO_PKG_VERSION")).as_str(),
        )
        .with_body(r#"{"data":{"data":{"A":"1"}}}"#)
        .create();
    let b = VaultBackend::new(VaultSettings {
        address: server.url(),
        namespace: None,
        ca_cert: None,
        token: "vt".into(),
    })
    .unwrap();
    assert!(b.get("m/p").unwrap().is_some());
    m.assert();
}

#[test]
fn custom_ca_is_loaded_and_bad_ca_rejected() {
    let dir = std::env::temp_dir().join(format!("diegops-ca-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let empty = dir.join("empty.pem");
    std::fs::write(&empty, "").unwrap();
    let err = VaultBackend::new(VaultSettings {
        address: "https://127.0.0.1:1".into(),
        namespace: None,
        ca_cert: Some(empty.clone()),
        token: "vt".into(),
    })
    .err()
    .unwrap()
    .to_string();
    assert_eq!(err, format!("no certificates found in {}", empty.display()));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn write_sends_plain_json_content_type() {
    let mut server = mockito::Server::new();
    server.mock("GET", "/v1/m/data/p").with_status(404).create();
    let w = server
        .mock("POST", "/v1/m/data/p")
        .match_header("content-type", "application/json")
        .with_body("{}")
        .expect(1)
        .create();
    let mut data = SecretMap::new();
    data.insert("X".into(), "v".into());
    backend(&server).put("m/p", &data).unwrap();
    w.assert();
}

#[test]
fn valid_custom_ca_builds_client() {
    use base64::Engine as _;
    let der = webpki_root_certs::TLS_SERVER_ROOT_CERTS[0].as_ref();
    let b64 = base64::engine::general_purpose::STANDARD.encode(der);
    let pem = format!("-----BEGIN CERTIFICATE-----\n{b64}\n-----END CERTIFICATE-----\n");
    let path = std::env::temp_dir().join(format!("diegops-ca-ok-{}.pem", std::process::id()));
    std::fs::write(&path, pem).unwrap();
    let res = VaultBackend::new(VaultSettings {
        address: "https://127.0.0.1:1".into(),
        namespace: None,
        ca_cert: Some(path.clone()),
        token: "vt".into(),
    });
    let _ = std::fs::remove_file(&path);
    assert!(res.is_ok());
}
