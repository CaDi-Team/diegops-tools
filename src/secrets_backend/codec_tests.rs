use super::*;

#[test]
fn plain_single_line_value_is_stored_verbatim() {
    assert_eq!(encode("s3cr3t-token"), "s3cr3t-token");
    assert_eq!(decode("s3cr3t-token").unwrap(), "s3cr3t-token");
}

#[test]
fn multi_line_value_is_base64_with_marker_and_round_trips() {
    let pem = "-----BEGIN PGP PRIVATE KEY BLOCK-----\nabc\n-----END PGP PRIVATE KEY BLOCK-----\n";
    let enc = encode(pem);
    assert!(enc.starts_with(MARKER));
    assert!(!enc.contains('\n'));
    assert_eq!(decode(&enc).unwrap(), pem);
}

#[test]
fn surrounding_whitespace_is_preserved_via_encoding() {
    // Infisical trims values server-side; encoding keeps them byte-exact.
    for v in [" leading", "trailing ", "\ttab", "x\r"] {
        let enc = encode(v);
        assert!(enc.starts_with(MARKER), "{v:?} should be encoded");
        assert_eq!(decode(&enc).unwrap(), v);
    }
}

#[test]
fn value_that_looks_like_marker_is_escaped() {
    let tricky = format!("{MARKER}not-really-base64");
    let enc = encode(&tricky);
    assert_ne!(enc, tricky);
    assert_eq!(decode(&enc).unwrap(), tricky);
}

#[test]
fn decode_rejects_invalid_base64_after_marker() {
    assert!(decode(&format!("{MARKER}%%%")).is_err());
}

#[test]
fn sha256_hex_is_stable() {
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}
