# `diegops bootstrap` — Design Spec

**Date:** 2026-03-30
**Status:** Approved

## Purpose

A single command that bootstraps a fresh workstation end-to-end: verifies authentication, pulls cloud config, clones repositories, injects secrets, and restores workstation files. One command, zero flags, full setup.

## Command

```
diegops bootstrap
```

No arguments. No flags. Uses default config paths for all sub-commands.

## Flow

Six numbered steps executed in order:

### Pre-flight (fatal — stop on first failure)

| Step | Action | Implementation |
|------|--------|----------------|
| 1/6 | Check GitHub CLI is authenticated | `auth::load_gh_token()` then HTTP GET `https://api.github.com/user` to get username |
| 2/6 | Check Vault CLI is authenticated | `common::check_vault_binary()` + `common::check_vault_addr()` + `common::check_vault_auth()` |

If either pre-flight check fails, print the error and exit with code 1 immediately. There is no value in continuing without both services authenticated.

### Execution (continue on error — collect results)

| Step | Action | Implementation |
|------|--------|----------------|
| 3/6 | Sync config from cloud | `sync::pull()` |
| 4/6 | Clone repositories | `repo::apply(None, None)` |
| 5/6 | Inject .env secrets | `vault::apply(None, None)` |
| 6/6 | Restore workstation files | `secrets::pull(None, None)` |

Each step is wrapped in a match. On `Err(e)`, the error message is captured, the step is marked as failed, and execution continues to the next step.

### Report

After all steps complete:

1. Print the hero banner (`cadi::run()`)
2. Print a per-step summary table showing OK or FAILED with the error message
3. Print a final status line: "Workstation ready." or "Workstation ready with N error(s)."
4. Exit with code 0 if all steps passed, code 1 if any execution step failed

## Output Format

### Success

```
[1/6] Checking GitHub CLI ... OK (dpinto)
[2/6] Checking Vault CLI ... OK
[3/6] Syncing config from cloud ... OK
[4/6] Cloning repositories ... OK
[5/6] Injecting .env secrets ... OK
[6/6] Restoring workstation files ... OK

<hero banner>

Summary:
  OK  GitHub CLI authenticated
  OK  Vault CLI authenticated
  OK  Config synced from cloud
  OK  Repositories cloned
  OK  .env secrets injected
  OK  Workstation files restored

Workstation ready.
```

### Partial failure

```
[1/6] Checking GitHub CLI ... OK (dpinto)
[2/6] Checking Vault CLI ... OK
[3/6] Syncing config from cloud ... OK
[4/6] Cloning repositories ... OK
[5/6] Injecting .env secrets ... FAILED
[6/6] Restoring workstation files ... OK

<hero banner>

Summary:
  OK  GitHub CLI authenticated
  OK  Vault CLI authenticated
  OK  Config synced from cloud
  OK  Repositories cloned
  FAIL  .env secrets: connection refused
  OK  Workstation files restored

Workstation ready with 1 error(s).
```

### Pre-flight failure

```
[1/6] Checking GitHub CLI ... FAILED
      not authenticated. Run 'diegops auth gh login <PAT>' first
```

Exits immediately with code 1.

## Module Structure

- **New file:** `src/commands/bootstrap.rs`
- **Single public function:** `pub fn run() -> Result<(), Box<dyn std::error::Error>>`
- **Clap variant:** `Commands::Bootstrap` in `src/main.rs`
- **Module registration:** add `pub mod bootstrap;` to `src/commands/mod.rs`

## Step result tracking

A simple struct to track each step's outcome:

```rust
struct StepResult {
    label: &'static str,
    ok: bool,
    detail: String,  // username on success, error message on failure
}
```

Collected into a `Vec<StepResult>` and printed in the summary.

## GitHub auth check

The bootstrap command needs the GitHub username for the "[1/6] ... OK (dpinto)" output. The existing `auth::gh_whoami()` prints directly to stdout and returns `()`. Rather than refactoring that function, bootstrap will replicate the minimal check:

1. `auth::load_gh_token()` — get the token (error if None)
2. HTTP GET `https://api.github.com/user` with the token — extract `login` field

This is ~10 lines and avoids changing the existing auth API.

## What is NOT included

- No `--skip-*` flags — run individual commands if you need granular control
- No `--config` overrides — uses defaults for everything
- No retry logic
- No interactive prompts
- No new dependencies

## Testing

- **Unit test:** verify `StepResult` summary formatting
- **Integration test:** `diegops bootstrap --help` exits successfully
