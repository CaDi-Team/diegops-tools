# Bootstrap Command Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `diegops bootstrap` command that verifies auth, pulls config, clones repos, injects secrets, and restores workstation files — all in one shot.

**Architecture:** Single new module `bootstrap.rs` that calls existing public functions from `auth`, `sync`, `repo`, `vault`, `secrets`, and `cadi`. Tracks per-step results and prints a summary report with the hero banner.

**Tech Stack:** Rust, clap derive, ureq (for GitHub API check)

**Spec:** `docs/superpowers/specs/2026-03-30-bootstrap-command-design.md`

---

## File Structure

| Action | File | Responsibility |
|--------|------|----------------|
| Create | `src/commands/bootstrap.rs` | Bootstrap orchestration logic |
| Modify | `src/commands/mod.rs:19` | Register bootstrap module |
| Modify | `src/main.rs:33-161` | Add `Bootstrap` variant to `Commands` enum and dispatch |
| Modify | `tests/integration_test.rs` | Add `bootstrap --help` integration test |

---

### Task 1: Register the module and wire clap

**Files:**
- Create: `src/commands/bootstrap.rs`
- Modify: `src/commands/mod.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Create bootstrap.rs with a stub `run()` function**

Create `src/commands/bootstrap.rs`:

```rust
//! The `diegops bootstrap` command — full workstation setup in one shot.

/// Bootstraps a fresh workstation: verifies auth, pulls config, clones repos,
/// injects secrets, and restores workstation files.
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    println!("bootstrap: not yet implemented");
    Ok(())
}
```

- [ ] **Step 2: Register the module in mod.rs**

In `src/commands/mod.rs`, add `pub mod bootstrap;` in alphabetical order (after `pub mod auth;`):

```rust
pub mod auth;
pub mod bootstrap;
pub mod cadi;
```

- [ ] **Step 3: Add the `Bootstrap` variant to the `Commands` enum in main.rs**

In `src/main.rs`, add the variant after `Cadi` (line 59):

```rust
    /// Show the DiegOps hero screen
    Cadi,
    /// Bootstrap a fresh workstation in one shot
    #[command(
        long_about = "Bootstrap a fresh workstation in one shot.\n\n\
            Verifies GitHub and Vault authentication, then runs:\n  \
            sync pull → repo apply → vault apply → secrets pull\n\n\
            Finishes with a summary report and the hero banner.\n\
            Requires: GitHub token and Vault session."
    )]
    Bootstrap,
```

- [ ] **Step 4: Add the dispatch arm in main()**

In `src/main.rs`, add the match arm after the `Cadi` arm (after line 178):

```rust
        Some(Commands::Cadi) => {
            commands::cadi::run();
        }
        Some(Commands::Bootstrap) => {
            commands::bootstrap::run()?;
        }
```

- [ ] **Step 5: Build and verify**

Run: `cargo build 2>&1`
Expected: compiles with no errors

- [ ] **Step 6: Commit**

```bash
git add src/commands/bootstrap.rs src/commands/mod.rs src/main.rs
git commit -m "feat(bootstrap): add stub command with clap wiring"
```

---

### Task 2: Implement pre-flight auth checks

**Files:**
- Modify: `src/commands/bootstrap.rs`

- [ ] **Step 1: Write the pre-flight check logic**

Replace the contents of `src/commands/bootstrap.rs` with:

```rust
//! The `diegops bootstrap` command — full workstation setup in one shot.

/// Bootstraps a fresh workstation: verifies auth, pulls config, clones repos,
/// injects secrets, and restores workstation files.
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    // -- Pre-flight: GitHub --------------------------------------------------
    eprint!("[1/6] Checking GitHub CLI ... ");
    let gh_user = check_github()?;
    eprintln!("OK ({gh_user})");

    // -- Pre-flight: Vault ---------------------------------------------------
    eprint!("[2/6] Checking Vault CLI ... ");
    check_vault()?;
    eprintln!("OK");

    eprintln!();
    println!("Pre-flight passed. Bootstrap not yet implemented.");
    Ok(())
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
```

- [ ] **Step 2: Build and verify**

Run: `cargo build 2>&1`
Expected: compiles with no errors

- [ ] **Step 3: Commit**

```bash
git add src/commands/bootstrap.rs
git commit -m "feat(bootstrap): implement pre-flight auth checks"
```

---

### Task 3: Implement execution steps and summary report

**Files:**
- Modify: `src/commands/bootstrap.rs`

- [ ] **Step 1: Replace bootstrap.rs with the full implementation**

Replace the full contents of `src/commands/bootstrap.rs` with:

```rust
//! The `diegops bootstrap` command — full workstation setup in one shot.

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
    let steps: Vec<(&str, &str, fn() -> Result<(), Box<dyn std::error::Error>>)> = vec![
        ("3/6", "Syncing config from cloud", || super::sync::pull()),
        ("4/6", "Cloning repositories", || {
            super::repo::apply(None, None)
        }),
        ("5/6", "Injecting .env secrets", || {
            super::vault::apply(None, None)
        }),
        ("6/6", "Restoring workstation files", || {
            super::secrets::pull(None, None)
        }),
    ];

    let labels = [
        "Config synced from cloud",
        "Repositories cloned",
        ".env secrets injected",
        "Workstation files restored",
    ];

    for (i, (step_num, description, action)) in steps.into_iter().enumerate() {
        eprint!("[{step_num}] {description} ... ");
        match action() {
            Ok(()) => {
                eprintln!("OK");
                results.push(StepResult {
                    label: labels[i],
                    ok: true,
                    detail: String::new(),
                });
            }
            Err(e) => {
                eprintln!("FAILED");
                results.push(StepResult {
                    label: labels[i],
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
    } else {
        eprintln!("Workstation ready with {failures} error(s).");
        std::process::exit(1);
    }
    Ok(())
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
```

- [ ] **Step 2: Build and verify**

Run: `cargo build 2>&1`
Expected: compiles with no errors

- [ ] **Step 3: Commit**

```bash
git add src/commands/bootstrap.rs
git commit -m "feat(bootstrap): implement execution steps and summary report"
```

---

### Task 4: Add integration test

**Files:**
- Modify: `tests/integration_test.rs`

- [ ] **Step 1: Add the --help integration test**

Append to `tests/integration_test.rs`:

```rust
#[test]
fn bootstrap_help_exits_successfully() {
    let output = diegops()
        .args(["bootstrap", "--help"])
        .output()
        .expect("failed to run diegops bootstrap --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Bootstrap"),
        "help should mention Bootstrap: {stdout}"
    );
}
```

- [ ] **Step 2: Run the test**

Run: `cargo test bootstrap_help -- --nocapture 2>&1`
Expected: test passes

- [ ] **Step 3: Commit**

```bash
git add tests/integration_test.rs
git commit -m "test(bootstrap): add integration test for --help"
```

---

### Task 5: Update CLAUDE.md command table

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Add bootstrap to the command table**

In `CLAUDE.md`, find the `## Commands` table and add after `diegops cadi`:

```markdown
| `diegops bootstrap` | Full workstation setup in one shot |
```

- [ ] **Step 2: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: add bootstrap command to CLAUDE.md"
```

---

### Task 6: Update README.md

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Add bootstrap to "Commands at a Glance"**

In `README.md`, in the "Commands at a Glance" code block, add after the `diegops help` line:

```
diegops bootstrap            Full workstation setup in one shot
```

- [ ] **Step 2: Add a dedicated bootstrap section**

In `README.md`, add a new section after "Commands at a Glance" and before the `diegops repo` section:

````markdown
## `diegops bootstrap` — Full Workstation Setup

Set up a fresh workstation with a single command. Verifies authentication, pulls cloud config, clones repositories, injects secrets, and restores workstation files.

```
diegops bootstrap
```

**What it does (in order):**

1. Checks GitHub CLI is authenticated
2. Checks Vault CLI is authenticated
3. Pulls config files from cloud (`sync pull`)
4. Clones all configured repositories (`repo apply`)
5. Injects `.env` secrets from Vault (`vault apply`)
6. Restores workstation files from Vault (`secrets pull`)

Finishes with the hero banner and a per-step summary. If any execution step fails, it continues with the remaining steps and reports all failures at the end.

**Requires:** GitHub token (`diegops auth gh login` first) and Vault session (`vault login` first).

---
````

- [ ] **Step 3: Review the full README for completeness and accuracy**

Read through the entire README and verify:
- All commands in the "Commands at a Glance" block match the actual CLI
- All section headings match available subcommands
- No stale information or missing features
- `secrets.yaml` section mentions the new template content (SSH, kube, workspaces)
- Directory layout section is up to date (includes `secrets.yaml`)
- Bootstrap section is in the right position

- [ ] **Step 4: Commit**

```bash
git add README.md
git commit -m "docs: add bootstrap command to README and review for completeness"
```

---

### Task 7: Version bump, tag, and push

**Files:**
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`

- [ ] **Step 1: Bump version**

In `Cargo.toml`, change `version = "1.1.6"` to `version = "1.2.0"` (new feature = minor bump).

Update `Cargo.lock` to match (`version = "1.2.0"` for the `diegops` entry).

- [ ] **Step 2: Commit, push, tag, push tag**

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore: bump version to 1.2.0"
git push
git tag v1.2.0
git push origin v1.2.0
```
