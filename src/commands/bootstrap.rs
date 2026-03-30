//! The `diegops bootstrap` command — full workstation setup in one shot.

/// A bootstrap step: (step number, progress description, summary label, action).
type Step = (
    &'static str,
    &'static str,
    &'static str,
    fn() -> Result<(), Box<dyn std::error::Error>>,
);

/// Tracks the outcome of a single bootstrap step.
struct StepResult {
    label: &'static str,
    ok: bool,
    detail: String,
}

/// Bootstraps a fresh workstation: verifies auth, pulls config, clones repos,
/// injects secrets, and restores workstation files.
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut results: Vec<StepResult> = Vec::new();

    // -- Pre-flight: GitHub --------------------------------------------------
    eprint!("[1/6] Checking GitHub CLI ... ");
    let gh_user = check_github()?;
    eprintln!("OK ({gh_user})");
    results.push(StepResult {
        label: "GitHub CLI authenticated",
        ok: true,
        detail: gh_user,
    });

    // -- Pre-flight: Vault ---------------------------------------------------
    eprint!("[2/6] Checking Vault CLI ... ");
    check_vault()?;
    eprintln!("OK");
    results.push(StepResult {
        label: "Vault CLI authenticated",
        ok: true,
        detail: String::new(),
    });

    // -- Execution steps (continue on error) ---------------------------------
    let steps: Vec<Step> = vec![
        (
            "3/6",
            "Syncing config from cloud",
            "Config synced from cloud",
            || super::sync::pull(),
        ),
        ("4/6", "Cloning repositories", "Repositories cloned", || {
            super::repo::apply(None, None)
        }),
        (
            "5/6",
            "Injecting .env secrets",
            ".env secrets injected",
            || super::vault::apply(None, None),
        ),
        (
            "6/6",
            "Restoring workstation files",
            "Workstation files restored",
            || super::secrets::pull(None, None),
        ),
    ];

    for (step_num, description, label, action) in steps {
        eprint!("[{step_num}] {description} ... ");
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

    // -- Report --------------------------------------------------------------
    eprintln!();
    super::cadi::run();
    eprintln!();

    let failures = print_summary(&results);

    eprintln!();
    if failures == 0 {
        eprintln!("Workstation ready.");
        Ok(())
    } else {
        eprintln!("Workstation ready with {failures} error(s).");
        Err(format!("{failures} bootstrap step(s) failed").into())
    }
}

/// Prints the summary table and returns the number of failures.
fn print_summary(results: &[StepResult]) -> usize {
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
