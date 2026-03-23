# GitHub Copilot Instructions — diegops

## What this project is

`diegops` is a personal productivity CLI written in **Rust**, targeting six platforms:
- `x86_64-pc-windows-gnu` (Windows terminal / WSL — GNU toolchain via cross/MinGW)
- `x86_64-apple-darwin` (macOS Intel)
- `aarch64-apple-darwin` (macOS Apple Silicon)
- `x86_64-unknown-linux-gnu` (Ubuntu / Debian)
- `x86_64-unknown-linux-musl` (Alpine Linux — static binary)
- `aarch64-unknown-linux-musl` (ARM64 Alpine — static binary)

It must work equally well for humans typing in a terminal and for automation inside containers.

## Stack

- Rust stable (no nightly features)
- `clap` v4 with derive macros for argument parsing
- Cargo for builds and tests
- GitHub Actions for CI (fmt + clippy + test) and releases (tag-triggered cross-platform builds)
- Self-hosted runner: **all** `runs-on` values must be `cadi-hq-runner-dind-set` — never use `ubuntu-latest`, `macos-latest`, or `windows-latest`
- Build tools: native cargo (linux-gnu), cross/Docker (linux-musl + windows-gnu), cargo-zigbuild (macOS)
- Windows target is `x86_64-pc-windows-gnu` — MSVC cross-compilation from Linux is not possible
- cross has no Docker image for macOS targets — use cargo-zigbuild for those

## Rules Copilot must follow

### Cross-platform paths
Always use `std::path::PathBuf` and `Path`. Never build paths by concatenating strings.

```rust
// Wrong
let config = format!("{}/config.toml", home);
// Right
let config = home_dir.join("config.toml");
```

### No shell assumptions
Do not call `sh`, `bash`, `cmd.exe`, or `powershell` implicitly. Any subprocess invocation needs OS detection.

### No .unwrap() in production paths
Use `?` to propagate errors. `main()` must return `Result<(), Box<dyn std::error::Error>>`.

```rust
// Wrong
let val = std::env::var("HOME").unwrap();
// Right
let val = std::env::var("HOME").map_err(|_| "HOME not set")?;
```

### OS-specific code needs cfg guards
```rust
#[cfg(target_os = "windows")]
fn platform_specific() { ... }
```

### Formatting and linting
Generated code must pass:
```
cargo fmt --check
cargo clippy -- -D warnings
```

No dead code, unused imports, or shadowed variables that trigger warnings.

### Exit codes
- `0` — success (idempotent success counts too: "already done" = exit 0)
- `1` — user/input error
- `2` — internal error
Never hardcode `std::process::exit()` outside `main`. Let `?` propagate and Rust's `main() -> Result` machinery handle it.

### stdout vs stderr
- **stdout** (`println!`): machine-readable output, the result of a command
- **stderr** (`eprintln!`): error messages, diagnostics, progress
This split is mandatory so that `diegops cmd | jq` and CI log capture work correctly.

### TTY / no-TTY (container compatibility)
The binary runs inside Docker containers without a TTY. Rules:
- Never emit interactive prompts — use flags/arguments only.
- For custom ANSI/color output, check `std::io::IsTerminal` before writing escape codes.
- `clap` respects `NO_COLOR` automatically; do not re-implement color suppression.
- Never assume stdin is interactive.

### Idempotency
Commands must be safe to run multiple times. Prefer upsert over create-only. Do not error on "already exists" states.

```rust
// Wrong: errors on second run
fs::create_dir(&path)?;
// Right: idempotent
fs::create_dir_all(&path)?;
```

### Tests
- Unit tests: `#[cfg(test)]` inline modules.
- Integration tests: use `env!("CARGO_BIN_EXE_diegops")` to invoke the binary via `std::process::Command`.
- No network calls in tests.
- Tests must verify exit codes, not just stdout content.

### Binary size
Profile settings in `Cargo.toml` optimize for size (`opt-level = "z"`, `lto = true`, `strip = true`). Do not pull in large dependencies for simple tasks — prefer `std` over crates when feasible.

## Project structure

```
src/
  main.rs              # CLI entrypoint — thin dispatcher only
  commands/            # One module per non-trivial command (extract when >~20 lines)
tests/
  integration_test.rs  # CLI integration tests via process::Command
Cargo.toml
Cargo.lock             # Always committed (binary crate)
```

## Commands

| Command | Description |
|---------|-------------|
| `diegops version` | Print version string |
| `diegops update` | Self-update from latest GitHub release |
| `diegops repo init` | Create sample config at `~/.diegops/repos.yaml` |
| `diegops repo apply [--path PREFIX] [--config FILE]` | Clone all missing repos (idempotent) |
| `diegops repo list [--config FILE]` | Show repos cloned locally |
| `diegops repo list-diff [--config FILE]` | Show repos in config but not cloned |
| `diegops help` | Show full help |

`diegops repo` reads `~/.diegops/repos.yaml` (override: `--config` or `$DIEGOPS_REPOS_CONFIG`). Format: `targets[]` with `path` (`$HOME`-prefixed) and `repos[]` (SSH git URLs). `apply` is idempotent; progress to stderr, summary to stdout.

`diegops update` specifics: target triple determined at compile time via `#[cfg]` in `src/commands/update.rs`; HTTP via `ureq` (rustls, sync); archives via `flate2`+`tar` (Unix) / `zip` (Windows); self-replace via atomic rename (Unix) or rename-to-`.old` (Windows).

## Adding a command

1. Add variant to `Commands` enum in `src/main.rs` with `///` doc comment.
2. Add match arm calling into `src/commands/<name>.rs` for non-trivial logic.
3. Add integration test in `tests/integration_test.rs`. Network-dependent commands: only test `--help` in the offline suite.
4. Update `README.md`.
