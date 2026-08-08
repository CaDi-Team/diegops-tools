//! The `diegops all` command — a single push/pull orchestrator across
//! `repo`, `secrets`, `vault`, and `sync`.
//!
//! `all pull` runs, in dependency-correct order: cloud config sync, SSH key
//! restore (needed before any repo can be cloned over SSH), repo cloning,
//! team Vault secrets, the rest of the personal secret files, and shell
//! setup. `all push` runs the two directions that have a push side:
//! personal secret files, then the `~/.diegops/` config files themselves.
//!
//! Every step runs even if an earlier one fails (continue-on-error), and a
//! summary is printed at the end — this matches the existing `bootstrap`
//! command's behavior and keeps the CLI usable unattended in CI.

/// A single step: (progress description, summary label, action).
type Step = (
    &'static str,
    &'static str,
    fn() -> Result<(), Box<dyn std::error::Error>>,
);

/// Outcome of one step in an `all pull`/`all push` run.
pub struct StepResult {
    pub label: &'static str,
    pub ok: bool,
    pub detail: String,
}

// ---------------------------------------------------------------------------
// Clap sub-command definitions
// ---------------------------------------------------------------------------

/// Unified push/pull sub-commands.
#[derive(clap::Subcommand)]
pub enum AllCommand {
    /// Pull everything: cloud config, SSH keys, repos, vault secrets, workstation files, shell.
    Pull,
    /// Push everything: personal secret files, then the ~/.diegops config.
    Push,
}

// ---------------------------------------------------------------------------
// Public entry points
// ---------------------------------------------------------------------------

/// Pulls everything needed to make a workstation usable, in dependency order.
///
/// GitHub and Vault auth are verified up front (fatal — returns immediately
/// on failure, before running any step). The six sync steps that follow
/// continue past individual failures; check the returned results for
/// per-step outcomes.
pub fn pull() -> Result<Vec<StepResult>, Box<dyn std::error::Error>> {
    eprint!("Checking GitHub CLI ... ");
    let gh_user = check_github()?;
    eprintln!("OK ({gh_user})");

    eprint!("Checking Vault CLI ... ");
    check_vault()?;
    eprintln!("OK");

    let steps: Vec<Step> = vec![
        (
            "Syncing config from cloud",
            "Config synced from cloud",
            || super::sync::pull(),
        ),
        ("Restoring SSH keys", "SSH keys restored", || {
            super::secrets::pull(None, Some("$HOME/.ssh"))
        }),
        ("Cloning repositories", "Repositories cloned", || {
            super::repo::apply(None, None)
        }),
        ("Injecting vault secrets", "Vault secrets injected", || {
            super::vault::apply(None, None)
        }),
        (
            "Restoring workstation files",
            "Workstation files restored",
            || super::secrets::pull(None, None),
        ),
        ("Configuring shell", "Shell configured", || {
            super::shell::init()
        }),
    ];

    Ok(run_steps(&steps))
}

/// Pushes everything with a push side: personal secret files, then the
/// `~/.diegops/` config files themselves.
///
/// `repo` has no push concept (that's `git push` inside each repo) and
/// `vault` is deliberately pull-only (team-owned namespace) — neither
/// appears here.
pub fn push() -> Result<Vec<StepResult>, Box<dyn std::error::Error>> {
    let steps: Vec<Step> = vec![
        ("Pushing secret files", "Secret files pushed", || {
            super::secrets::push(None, None)
        }),
        ("Pushing config to cloud", "Config pushed to cloud", || {
            super::sync::push()
        }),
    ];

    Ok(run_steps(&steps))
}

/// Prints an `OK`/`FAILED` summary line per step and returns the failure count.
pub fn print_summary(results: &[StepResult]) -> usize {
    let mut failures = 0;
    eprintln!("Summary:");
    for step in results {
        if step.ok {
            eprintln!("  OK    {}", step.label);
        } else {
            eprintln!("  FAIL  {}: {}", step.label, step.detail);
            failures += 1;
        }
    }
    failures
}

/// Prints the summary and converts a nonzero failure count into an error.
///
/// `noun` names the run in the final message, e.g. `"pull"` or `"push"`.
pub fn finish(results: &[StepResult], noun: &str) -> Result<(), Box<dyn std::error::Error>> {
    let failures = print_summary(results);
    if failures == 0 {
        Ok(())
    } else {
        Err(format!("{failures} {noun} step(s) failed").into())
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Runs each step in order, printing `[i/n] <description> ... OK|FAILED`,
/// continuing past a failed step.
fn run_steps(steps: &[Step]) -> Vec<StepResult> {
    let total = steps.len();
    let mut results = Vec::with_capacity(total);
    for (i, (description, label, action)) in steps.iter().enumerate() {
        eprint!("[{}/{total}] {description} ... ", i + 1);
        match action() {
            Ok(()) => {
                eprintln!("OK");
                results.push(StepResult {
                    label,
                    ok: true,
                    detail: String::new(),
                });
            }
            Err(e) => {
                eprintln!("FAILED");
                results.push(StepResult {
                    label,
                    ok: false,
                    detail: e.to_string(),
                });
            }
        }
    }
    results
}

/// Verifies GitHub authentication and returns the username.
fn check_github() -> Result<String, Box<dyn std::error::Error>> {
    let token = super::auth::load_gh_token()?
        .ok_or("not authenticated. Run 'diegops auth gh login <PAT>' first")?;

    let ua = format!("diegops/{}", env!("CARGO_PKG_VERSION"));
    let response = ureq::get("https://api.github.com/user")
        .set("User-Agent", &ua)
        .set("Authorization", &format!("Bearer {token}"))
        .set("Accept", "application/vnd.github.v3+json")
        .call();

    match response {
        Ok(resp) => {
            let json: serde_json::Value = resp.into_json()?;
            let login = json["login"]
                .as_str()
                .ok_or("GitHub API response missing 'login'")?;
            Ok(login.to_string())
        }
        Err(ureq::Error::Status(401 | 403, _)) => {
            Err("stored GitHub token is no longer valid. Run 'diegops auth gh login <PAT>'".into())
        }
        Err(ureq::Error::Status(code, _)) => {
            Err(format!("GitHub API returned unexpected status {code}").into())
        }
        Err(ureq::Error::Transport(_)) => {
            Err("could not reach GitHub API. Check your connection".into())
        }
    }
}

/// Verifies Vault CLI is available and authenticated.
fn check_vault() -> Result<(), Box<dyn std::error::Error>> {
    super::common::check_vault_binary()?;
    super::common::check_vault_addr()?;
    super::common::check_vault_auth()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_steps_reports_ok_for_succeeding_steps() {
        let steps: Vec<Step> = vec![("desc", "label", || Ok(()))];
        let results = run_steps(&steps);
        assert_eq!(results.len(), 1);
        assert!(results[0].ok);
        assert_eq!(results[0].label, "label");
        assert_eq!(results[0].detail, "");
    }

    #[test]
    fn run_steps_reports_failure_detail_for_failing_steps() {
        let steps: Vec<Step> = vec![("desc", "label", || Err("boom".into()))];
        let results = run_steps(&steps);
        assert_eq!(results.len(), 1);
        assert!(!results[0].ok);
        assert_eq!(results[0].detail, "boom");
    }

    #[test]
    fn run_steps_continues_past_a_failure() {
        let steps: Vec<Step> = vec![
            ("desc1", "label1", || Err("boom".into())),
            ("desc2", "label2", || Ok(())),
        ];
        let results = run_steps(&steps);
        assert_eq!(results.len(), 2);
        assert!(!results[0].ok);
        assert!(results[1].ok);
    }

    #[test]
    fn print_summary_counts_failures() {
        let results = vec![
            StepResult {
                label: "a",
                ok: true,
                detail: String::new(),
            },
            StepResult {
                label: "b",
                ok: false,
                detail: "bad".to_string(),
            },
        ];
        assert_eq!(print_summary(&results), 1);
    }

    #[test]
    fn finish_ok_when_no_failures() {
        let results = vec![StepResult {
            label: "a",
            ok: true,
            detail: String::new(),
        }];
        assert!(finish(&results, "pull").is_ok());
    }

    #[test]
    fn finish_errs_when_any_failure() {
        let results = vec![StepResult {
            label: "a",
            ok: false,
            detail: "bad".to_string(),
        }];
        let err = finish(&results, "push").unwrap_err();
        assert!(err.to_string().contains("1 push step(s) failed"));
    }
}
