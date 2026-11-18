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
