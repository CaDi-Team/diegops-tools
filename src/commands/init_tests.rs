use super::*;
use std::cell::RefCell;
use std::collections::VecDeque;

/// Scripted prompter: pops answers in order; records what was asked.
struct Script {
    answers: VecDeque<String>,
    asked: Vec<String>,
}

impl Script {
    fn new(a: &[&str]) -> Self {
        Self {
            answers: a.iter().map(|s| s.to_string()).collect(),
            asked: Vec::new(),
        }
    }
    fn next(&mut self, f: &Field) -> Result<String, BoxError> {
        self.asked.push(f.key.to_string());
        self.answers.pop_front().ok_or_else(|| missing(f))
    }
}

impl Prompter for Script {
    fn text(&mut self, f: &Field, _d: Option<&str>) -> Result<String, BoxError> {
        self.next(f)
    }
    fn secret(&mut self, f: &Field) -> Result<String, BoxError> {
        self.next(f)
    }
    fn select(&mut self, f: &Field, o: &[&str], _d: usize) -> Result<usize, BoxError> {
        let a = self.next(f)?;
        Ok(o.iter().position(|x| *x == a).unwrap())
    }
}

fn no_env(_: &str) -> Option<String> {
    None
}

struct World {
    good_secret: &'static str,
    good_pat: &'static str,
    existing_pat: Option<(String, String)>,
    seen_secrets: RefCell<Vec<Option<String>>>,
}

impl World {
    fn new() -> Self {
        Self {
            good_secret: "s3cret",
            good_pat: "ghp_ok",
            existing_pat: None,
            seen_secrets: RefCell::new(Vec::new()),
        }
    }
    fn run(
        &self,
        given: &Given,
        existing: Option<&Config>,
        p: &mut dyn Prompter,
        interactive: bool,
    ) -> Result<Plan, BoxError> {
        let vb = |cfg: &Config, kind: BackendKind, env: &dyn Fn(&str) -> Option<String>| match kind
        {
            BackendKind::Vault => Ok(()),
            BackendKind::Infisical => {
                let s = env("INFISICAL_UNIVERSAL_AUTH_CLIENT_SECRET");
                self.seen_secrets.borrow_mut().push(s.clone());
                let ok = s.as_deref() == Some(self.good_secret)
                    || (s.is_none() && cfg.infisical.client_secret_file.as_deref() == Some("/ok"));
                if ok {
                    Ok(())
                } else {
                    Err("login rejected (401)".into())
                }
            }
        };
        let vg = |t: &str| {
            if t == self.good_pat {
                Ok("diego".to_string())
            } else {
                Err::<String, BoxError>("bad credentials".into())
            }
        };
        let eg = || self.existing_pat.clone();
        let d = Deps {
            validate_backend: &vb,
            validate_github: &vg,
            existing_github: &eg,
            env: &no_env,
            home: Path::new("/home/u"),
        };
        plan(
            given,
            existing,
            Path::new("/home/u/.config/diegops/config.toml"),
            p,
            interactive,
            &d,
        )
    }
}

#[test]
fn every_field_has_a_flag_and_env_var_on_init() {
    let cmd = crate::cli_command();
    let init = cmd.find_subcommand("init").expect("init command");
    for f in FIELDS {
        let arg = init
            .get_arguments()
            .chain(cmd.get_arguments())
            .find(|a| a.get_long() == Some(f.flag))
            .unwrap_or_else(|| panic!("field {} has no --{}", f.key, f.flag));
        if f.key != "client_secret" && f.key != "backend" {
            assert_eq!(
                arg.get_env().and_then(|e| e.to_str()),
                Some(f.env),
                "--{} must read {}",
                f.flag,
                f.env
            );
        }
    }
}

#[test]
fn rendered_config_parses_back() {
    let a = Answers {
        site_url: "https://eu.infisical.com".into(),
        project_id: "p\"1".into(),
        client_id: "cid".into(),
        client_secret_file: "~/.config/diegops/infisical-client-secret".into(),
        vault_addr: String::new(),
    };
    let cfg = config::parse(&render_config(BackendKind::Infisical, &a)).unwrap();
    assert_eq!(cfg.backend.as_deref(), Some("infisical"));
    assert_eq!(cfg.infisical.project_id.as_deref(), Some("p\"1"));
    assert_eq!(
        cfg.infisical.site_url.as_deref(),
        Some("https://eu.infisical.com")
    );
    let a = Answers {
        vault_addr: "https://bao.test".into(),
        ..Default::default()
    };
    let cfg = config::parse(&render_config(BackendKind::Vault, &a)).unwrap();
    assert_eq!(cfg.vault.address.as_deref(), Some("https://bao.test"));
}

#[test]
fn interactive_first_run_retries_the_secret_and_never_reviews_it() {
    let w = World::new();
    let mut p = Script::new(&[
        "infisical",
        "https://app.infisical.com",
        "proj",
        "cid",
        "wrong",
        "s3cret",
        "ghp_bad",
        "ghp_ok",
    ]);
    let plan = w.run(&Given::default(), None, &mut p, true).unwrap();
    let (path, secret) = plan.secret_file.as_ref().unwrap();
    assert_eq!(
        path,
        Path::new("/home/u/.config/diegops/infisical-client-secret")
    );
    assert_eq!(secret, "s3cret");
    assert_eq!(plan.github_token.as_deref(), Some("ghp_ok"));
    let text = plan.config_text.as_ref().unwrap();
    assert!(!text.contains("s3cret"));
    assert!(!text.contains("site_url"), "default site is not written");
    let review = plan.review.join("\n");
    assert!(
        !review.contains("s3cret") && !review.contains("ghp_ok"),
        "{review}"
    );
    assert!(review.contains("diego"), "{review}");
}

#[test]
fn gives_up_after_three_tries() {
    let w = World::new();
    let mut p = Script::new(&["infisical", "u", "p", "c", "a", "b", "c"]);
    let e = w.run(&Given::default(), None, &mut p, true).err().unwrap();
    assert!(e.to_string().contains("401"), "{e}");
    assert_eq!(w.seen_secrets.borrow().len(), 3);
}

#[test]
fn no_input_uses_flags_and_env_and_names_what_is_missing() {
    let w = World::new();
    let given = Given {
        backend: Some("infisical".into()),
        project_id: Some("proj".into()),
        client_secret: Some("s3cret".into()),
        skip_github: true,
        ..Default::default()
    };
    let e = w
        .run(&given, None, &mut NoPrompt, false)
        .err()
        .unwrap()
        .to_string();
    assert!(
        e.contains("--client-id") && e.contains("INFISICAL_UNIVERSAL_AUTH_CLIENT_ID"),
        "{e}"
    );

    let given = Given {
        client_id: Some("cid".into()),
        ..given
    };
    let plan = w.run(&given, None, &mut NoPrompt, false).unwrap();
    assert!(plan.config_text.is_some());
    assert!(plan.github_token.is_none());
}

#[test]
fn existing_secret_file_is_validated_not_rewritten() {
    let w = World::new();
    let given = Given {
        backend: Some("infisical".into()),
        project_id: Some("proj".into()),
        client_id: Some("cid".into()),
        client_secret_file: Some("/ok".into()),
        skip_github: true,
        ..Default::default()
    };
    let plan = w.run(&given, None, &mut NoPrompt, false).unwrap();
    assert!(plan.secret_file.is_none());
    assert!(plan
        .config_text
        .unwrap()
        .contains("client_secret_file = \"/ok\""));
}

#[test]
fn working_setup_is_skipped_unless_reset() {
    let mut w = World::new();
    w.existing_pat = Some(("ghp_ok".into(), "$GITHUB_TOKEN".into()));
    let cfg = config::parse("backend = \"vault\"\n[vault]\naddress = \"https://b\"\n").unwrap();
    let mut p = Script::new(&[]);
    let plan = w.run(&Given::default(), Some(&cfg), &mut p, true).unwrap();
    assert!(plan.config_text.is_none() && plan.github_token.is_none());
    assert!(p.asked.is_empty(), "asked {:?}", p.asked);
    let review = plan.review.join("\n");
    assert!(
        review.contains("✓ backend") && review.contains("✓ github token  diego via $GITHUB_TOKEN")
    );

    let given = Given {
        reset: vec!["backend".into()],
        ..Default::default()
    };
    let mut p = Script::new(&["vault", "https://bao.new"]);
    let plan = w.run(&given, Some(&cfg), &mut p, true).unwrap();
    assert!(plan.config_text.unwrap().contains("https://bao.new"));
}

#[test]
fn rejected_existing_github_token_is_an_error_with_a_hint() {
    let mut w = World::new();
    w.existing_pat = Some(("ghp_bad".into(), "$GH_TOKEN".into()));
    let given = Given {
        backend: Some("vault".into()),
        vault_addr: Some("https://b".into()),
        ..Default::default()
    };
    let e = w
        .run(&given, None, &mut NoPrompt, false)
        .err()
        .unwrap()
        .to_string();
    assert!(e.contains("$GH_TOKEN") && e.contains("hint:"), "{e}");
}

#[cfg(unix)]
#[test]
fn apply_writes_private_files_and_backs_up_config() {
    use std::os::unix::fs::PermissionsExt;
    let dir = std::env::temp_dir().join(format!("diegops-init-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let cfg = dir.join("config.toml");
    std::fs::write(&cfg, "old").unwrap();
    let plan = Plan {
        config_path: cfg.clone(),
        config_text: Some("backend = \"vault\"\n".into()),
        secret_file: Some((dir.join("sub/secret"), "x".into())),
        ..Default::default()
    };
    apply(&plan).unwrap();
    let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&cfg), 0o600);
    assert_eq!(mode(&dir.join("sub/secret")), 0o600);
    assert_eq!(
        std::fs::read_to_string(dir.join("config.toml.bak")).unwrap(),
        "old"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn reset_backend_defaults_to_current_values() {
    let w = World::new();
    let cfg = config::parse(
        "[infisical]\nproject_id = \"proj\"\nclient_id = \"cid\"\nclient_secret_file = \"/bad\"\n",
    )
    .unwrap();
    let given = Given {
        client_secret: Some("s3cret".into()),
        reset: vec!["backend".into()],
        skip_github: true,
        ..Default::default()
    };
    let plan = w.run(&given, Some(&cfg), &mut NoPrompt, false).unwrap();
    let text = plan.config_text.unwrap();
    assert!(
        text.contains("project_id = \"proj\"") && text.contains("client_id = \"cid\""),
        "{text}"
    );
}
