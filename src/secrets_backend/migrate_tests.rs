use super::*;
use std::cell::RefCell;
use std::collections::BTreeMap;

/// In-memory Vault-like backend.
#[derive(Default)]
struct Mem {
    data: RefCell<BTreeMap<String, SecretMap>>,
    writes: RefCell<usize>,
    sanitize: bool,
}

impl Mem {
    fn with(entries: &[(&str, &[(&str, &str)])]) -> Self {
        let m = Mem::default();
        for (p, kv) in entries {
            m.data.borrow_mut().insert(
                p.to_string(),
                kv.iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
            );
        }
        m
    }
}

impl SecretsBackend for Mem {
    fn name(&self) -> &'static str {
        "mem"
    }
    fn get(&self, path: &str) -> Result<Option<SecretMap>, BoxError> {
        Ok(self.data.borrow().get(path).cloned())
    }
    fn list(&self, path: &str) -> Result<Vec<String>, BoxError> {
        let prefix = format!("{}/", path.trim_end_matches('/'));
        let mut out: Vec<String> = Vec::new();
        for k in self.data.borrow().keys() {
            if let Some(rest) = k.strip_prefix(&prefix) {
                let item = match rest.split_once('/') {
                    Some((dir, _)) => format!("{dir}/"),
                    None => rest.to_string(),
                };
                if !out.contains(&item) {
                    out.push(item);
                }
            }
        }
        Ok(out)
    }
    fn put(&self, path: &str, data: &SecretMap) -> Result<(), BoxError> {
        *self.writes.borrow_mut() += 1;
        let mut d = self.data.borrow_mut();
        let e = d.entry(path.to_string()).or_default();
        for (k, v) in data {
            e.insert(k.clone(), v.clone());
        }
        Ok(())
    }
    fn validate_key(&self, key: &str) -> Result<(), BoxError> {
        if self.sanitize && key.contains(':') {
            return Err(format!("bad key '{key}'").into());
        }
        Ok(())
    }
    fn location_id(&self, path: &str) -> Result<String, BoxError> {
        Ok(if self.sanitize {
            path.replace('.', "_")
        } else {
            path.to_string()
        })
    }
}

fn source() -> Mem {
    Mem::with(&[
        (
            "dpinto-work/gnupg/main",
            &[("private.asc", "-----BEGIN-----\nAAA\n-----END-----\n")],
        ),
        (
            "dpinto-work/ssh/id",
            &[
                ("id_ed25519", "KEY\n"),
                ("id_ed25519.pub", "ssh-ed25519 AAA"),
            ],
        ),
        ("dpinto-config/app/env", &[("TOKEN", "t0p")]),
    ])
}

#[test]
fn collect_paths_walks_recursively() {
    let src = source();
    let paths = collect_paths(&src, "dpinto-work").unwrap();
    assert_eq!(paths, vec!["dpinto-work/gnupg/main", "dpinto-work/ssh/id"]);
}

#[test]
fn collect_paths_accepts_a_leaf() {
    let src = source();
    assert_eq!(
        collect_paths(&src, "dpinto-config/app/env").unwrap(),
        vec!["dpinto-config/app/env"]
    );
}

#[test]
fn dry_run_plans_but_writes_nothing() {
    let src = source();
    let dst = Mem::default();
    let r = migrate(&src, &dst, &["dpinto-work".into()], true).unwrap();
    assert_eq!(*dst.writes.borrow(), 0);
    assert_eq!(r.iter().map(|p| p.create).sum::<usize>(), 3);
    assert!(r.iter().all(|p| p.verified.is_none()));
}

#[test]
fn run_copies_verifies_and_is_idempotent() {
    let src = source();
    let dst = Mem::default();
    let r = migrate(
        &src,
        &dst,
        &["dpinto-work".into(), "dpinto-config".into()],
        false,
    )
    .unwrap();
    assert!(r.iter().all(|p| p.verified == Some(true)));
    assert_eq!(
        dst.get("dpinto-work/ssh/id").unwrap(),
        src.get("dpinto-work/ssh/id").unwrap()
    );
    let writes = *dst.writes.borrow();
    let again = migrate(
        &src,
        &dst,
        &["dpinto-work".into(), "dpinto-config".into()],
        false,
    )
    .unwrap();
    assert_eq!(*dst.writes.borrow(), writes, "second run must not write");
    assert!(again
        .iter()
        .all(|p| p.unchanged > 0 && p.create == 0 && p.update == 0));
}

#[test]
fn changed_source_value_is_an_update() {
    let src = source();
    let dst = Mem::with(&[("dpinto-config/app/env", &[("TOKEN", "old")])]);
    let r = migrate(&src, &dst, &["dpinto-config".into()], true).unwrap();
    assert_eq!(r[0].update, 1);
}

#[test]
fn colliding_targets_abort_before_writing() {
    let src = Mem::with(&[("m/a.b", &[("K", "1")]), ("m/a_b", &[("K", "2")])]);
    let dst = Mem {
        sanitize: true,
        ..Default::default()
    };
    assert!(migrate(&src, &dst, &["m".into()], false).is_err());
    assert_eq!(*dst.writes.borrow(), 0);
}

#[test]
fn report_never_contains_values() {
    let src = source();
    let dst = Mem::default();
    let r = migrate(
        &src,
        &dst,
        &["dpinto-work".into(), "dpinto-config".into()],
        false,
    )
    .unwrap();
    let text = render(&r);
    for v in ["t0p", "AAA", "KEY", "ssh-ed25519"] {
        assert!(!text.contains(v), "report leaked {v}: {text}");
    }
    assert!(text.contains("dpinto-config/app/env"));
}

#[test]
fn invalid_target_key_aborts_before_any_write() {
    let src = Mem::with(&[("m/a", &[("ok", "1")]), ("m/b", &[("bad:key", "2")])]);
    let dst = Mem {
        sanitize: true,
        ..Mem::default()
    };
    let err = migrate(&src, &dst, &["m".to_string()], false)
        .unwrap_err()
        .to_string();
    assert!(err.contains("bad:key"), "{err}");
    assert_eq!(*dst.writes.borrow(), 0);
}
