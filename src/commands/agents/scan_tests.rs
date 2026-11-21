use super::*;

fn hits(text: &str) -> Vec<Hit> {
    scan_text(text, &Allowlist::default())
}

#[test]
fn detects_common_credentials() {
    for line in [
        "OPENAI=sk-abcdefghijklmnopqrstuvwxyz012345",
        "token ghp_abcdefghijklmnopqrstuvwxyz0123456789",
        "github_pat_abcdefghijklmnopqrstuvwxyz0123456789",
        "key: AKIAABCDEFGHIJKLMNOP",
        "-----BEGIN OPENSSH PRIVATE KEY-----",
        "bot 123456789:AAabcdefghijklmnopqrstuvwxyz0123456789",
        "hvs.abcdefghijklmnopqrstuvwxyz",
        "api_key: abcdefghijklmnop1234",
        "PASSWORD = \"Abcdefghijklmnop1234\"",
    ] {
        assert_eq!(hits(line).len(), 1, "should flag: {line}");
    }
}

#[test]
fn ignores_clean_and_commented_lines() {
    assert!(hits("model: claude-opus\nprovider: copilot\n").is_empty());
    assert!(hits("# api_key: abcdefghijklmnop1234").is_empty());
    assert!(hits("  # sk-abcdefghijklmnopqrstuvwxyz012345").is_empty());
    // A word merely ending in "sk-" is not a key prefix.
    assert!(hits("task-abcdefghijklmnopqrstuvwxyz").is_empty());
}

#[test]
fn hit_reports_line_number_and_never_the_value() {
    let h = hits("ok\nOPENAI=sk-abcdefghijklmnopqrstuvwxyz012345\n");
    assert_eq!(h[0].line, 2);
    assert!(!h[0].to_string().contains("abcdefghij"));
}

#[test]
fn allowlist_hash_matches_legacy_scanner_format() {
    // Legacy scan-secrets.sh stores sha256(line) as lowercase hex.
    let line = "api_key: abcdefghijklmnop1234";
    let allow = Allowlist::parse(&format!("{}\n", line_hash(line)));
    assert!(scan_text(line, &allow).is_empty());
    assert_eq!(
        line_hash("abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn binary_content_is_scanned_lossily() {
    let mut bytes = vec![0u8, 159, 146, 150];
    bytes.extend_from_slice(b"\nsk-abcdefghijklmnopqrstuvwxyz012345\n");
    assert_eq!(scan_bytes(&bytes, &Allowlist::default()).len(), 1);
}
