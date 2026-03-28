# CLAUDE.md — diegops

## Project overview

`diegops` is a personal productivity CLI written in **Rust**, designed for first-class use across:

| Platform | Targets |
|----------|---------|
| Windows (WSL + native terminal) | `x86_64-pc-windows-gnu` |
| macOS Intel | `x86_64-apple-darwin` |
| macOS Apple Silicon | `aarch64-apple-darwin` |
| Ubuntu / Debian | `x86_64-unknown-linux-gnu` |
| Alpine (containers) | `x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl` |

Both interactive human use and container/CI workflow automation are first-class use cases. The binary must run standalone with no external runtime.

## Stack

- **Language**: Rust (stable channel — do not use nightly features)
- **CLI framework**: `clap` v4 with derive macros
- **Build**: Cargo
- **CI**: GitHub Actions (`cargo fmt`, `cargo clippy`, `cargo test`)
- **Releases**: GitHub Actions on `vx.y.z` tags → GitHub Release artifacts

## Repository structure

```
diegops-tools/
├── src/
│   └── main.rs              # CLI entry point and command definitions
├── tests/
│   └── integration_test.rs  # End-to-end CLI tests via process::Command
├── Cargo.toml
├── Cargo.lock               # Always committed — this is a binary crate
└── .github/
    └── workflows/
        ├── ci.yml           # PR gate: fmt + clippy + test
        └── cd.yml           # Release: builds all targets on vx.y.z tag push
```

## Core principles

### Idempotency
Every command must be safe to run multiple times with identical results. Never produce an error on a second run for state that already exists. Prefer upsert semantics over create-only.

### Cross-platform compatibility
- **Paths**: always use `std::path::PathBuf` / `Path`. Never concatenate strings with `/` or `\`.
- **Env vars**: use `std::env::var("NAME")` — never assume POSIX or Windows env conventions.
- **No shell assumptions**: do not call `sh`, `bash`, `cmd.exe`, or `powershell` implicitly. If a shell is needed, detect the OS and document why.
- **Platform guards**: any OS-specific code requires a `#[cfg(target_os = "...")]` attribute with a comment explaining the divergence.
- **Terminal output**: use `println!` / `eprintln!`. Avoid raw ANSI escape codes unless guarded by terminal detection (`TERM`, `NO_COLOR`, `isatty`).
- **Line endings**: rely on Rust's `println!` — do not manually write `\r\n`.
- **Static linking for musl**: Alpine/container builds use `x86_64-unknown-linux-musl` (static binary). Ensure no dynamic glibc dependencies in shared logic.

### Exit codes
Every command must exit with a meaningful code — containers and CI scripts check this:
- `0` — success (command completed, state is as desired — even if already was)
- `1` — user error (bad arguments, file not found, etc.)
- `2` — internal / unexpected error
Never call `std::process::exit()` outside `main`. Propagate errors with `?` and let `main` determine the exit code.

### stdout vs stderr
- **stdout** (`println!`) — command output and data. Pipelines and scripts read this.
- **stderr** (`eprintln!`) — errors, warnings, and progress messages. Never mix.
- A command that produces machine-readable output must write it exclusively to stdout so that `diegops cmd | jq` works correctly.

### TTY / no-TTY awareness
The binary must work correctly without a TTY (containers, CI, piped output):
- Never emit interactive prompts. Use flags/arguments instead.
- Color and styling: `clap` respects `NO_COLOR` automatically. For any custom ANSI output, check `std::io::IsTerminal` before emitting escape codes.
- Do not assume stdin is a terminal. Never call `stdin().read_line()` without a clear flag opt-in.

### Idempotency patterns
Concrete rules for safe repeated execution:
- File creation: use `fs::create_dir_all` (not `fs::create_dir`); write files only if content differs.
- "Already exists" states must succeed silently, not error.
- Log idempotent operations at the info/debug level (to stderr), not to stdout.

### Code quality gates
All code must pass:
```sh
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

Additional rules:
- No `.unwrap()` in production code paths — use `?` or explicit error handling.
- `main()` should return `Result<(), Box<dyn std::error::Error>>` and use `?` to propagate.
- All public items must have doc comments (`///`).
- Errors are `Result<T, E>` — bubble up with `?`, handle at the call site or in `main`.
- Keep `main()` as a thin dispatcher; extract non-trivial command logic to `src/commands/<name>.rs`.

### Testing
- **Unit tests**: `#[cfg(test)]` blocks inline with the code.
- **Integration tests**: `tests/` directory, using `std::process::Command` to invoke the compiled binary.
- Tests must run **offline** — no network calls. `diegops update` is the one exception; test only its `--help` flag in the integration suite.
- Tests must pass on **all target platforms** (write portable test code).
- Use `env!("CARGO_BIN_EXE_diegops")` in integration tests to locate the binary.
- Always assert `output.status.success()` or check the specific exit code — don't only check stdout.

## Commands

| Command | Description |
|---------|-------------|
| `diegops version` | Print version string |
| `diegops update` | Fetch latest GitHub release and replace binary in place |
| `diegops repo init` | Create sample config at `~/.diegops/repos.yaml` (idempotent) |
| `diegops repo apply [--path PREFIX] [--config FILE]` | Clone all missing repos (idempotent) |
| `diegops repo list [--config FILE]` | Show repos that are currently cloned locally |
| `diegops repo list-diff [--config FILE]` | Show repos in config but not cloned locally |
| `diegops vault init` | Create sample config at `~/.diegops/repo-vault.yaml` (idempotent) |
| `diegops vault apply [--path PREFIX] [--config FILE]` | Pull secrets from Vault and write `.env` files (idempotent) |
| `diegops vault list [--config FILE]` | Show targets that already have a `.env` file |
| `diegops vault list-diff [--config FILE]` | Show targets in config but missing `.env` |
| `diegops auth gh login <PAT>` | Validate and store a GitHub personal access token |
| `diegops auth gh logout` | Remove stored GitHub token |
| `diegops auth gh whoami` | Show authenticated GitHub user and token scopes |
| `diegops auth status` | Show authentication status for all providers |
| `diegops auth logout` | Remove all stored tokens |
| `diegops cadi` | Show the DiegOps hero screen |
| `diegops devtools git set [--name NAME] [--email EMAIL]` | Set git global user.name and user.email |
| `diegops devtools gpg init [--name NAME] [--email EMAIL]` | Generate GPG identity config |
| `diegops devtools gpg set` | Generate GPG key, upload to GitHub, configure signing |
| `diegops devtools gpg restart` | Restart gpg-agent |
| `diegops devtools ssh list` | List local and GitHub SSH keys |
| `diegops devtools ssh config` | Print `~/.ssh/config` |
| `diegops devtools ssh create [--name N] [--type T] [--email E]` | Create a new SSH key |
| `diegops help` | Show full help |

### `diegops repo` — workspace management
- Config file: `~/.diegops/repos.yaml` (override: `--config` or `$DIEGOPS_REPOS_CONFIG`)
- `init` creates `~/.diegops/repos.yaml` with a commented sample; skips silently if already exists
- Format: `targets[]` with `path` (`$HOME`-prefixed) and `repos[]` (SSH git URLs)
- `apply` is idempotent: existing `.git` dirs are skipped, target dirs are created if missing
- `--path` filter: only process targets whose expanded path starts with the given prefix; `$HOME` and `/$HOME` both accepted
- Progress (SKIP/CLONE/FAIL) → stderr; final summary → stdout
- Runs `git clone` via `std::process::Command` — no git crate needed; requires `git` on `$PATH`

### `diegops vault` — Vault secret management
- Config file: `~/.diegops/repo-vault.yaml` (override: `--config` or `$DIEGOPS_VAULT_CONFIG`)
- `init` creates `~/.diegops/repo-vault.yaml` with a commented sample; skips silently if already exists
- Format:
  ```yaml
  targets:
    - path: $HOME/github/org/my-app
      secrets:
        - vault_path: secret/my-app/database
          keys:
            - username
            - password
        - vault_path: secret/my-app/api
          keys: "*"
  ```
- `apply` behaviour:
  - Pre-flight checks: `vault` binary on PATH, `VAULT_ADDR` set, valid token (`vault token lookup`)
  - Writes `.env` files in `KEY="value"` format (double-quoted, escaped)
  - Automatically adds `.env` to `.gitignore` if not already present
  - Content-aware skip: if `.env` already matches, the write is skipped (idempotent)
  - On Unix, `.env` is created with `0600` permissions
- `vault_path` is the **logical** Vault path (no `/data/` segment — KV v2 adds it automatically)
- `keys`: a list of specific key names, or `"*"` to pull all keys; key names are case-preserved
- `--path` filter: only process targets whose expanded path starts with the given prefix
- Progress (SKIP/WRITE/FAIL) → stderr; final summary → stdout
- Shells out to `vault kv get -format=json` via `std::process::Command` — no Vault crate needed; requires `vault` on `$PATH`

### `diegops auth` — credential management
- Token storage: `~/.diegops/tokens/gh.json` (JSON with `version`, `token`, `stored_at` fields)
- Token resolution order: stored file > `$GITHUB_TOKEN` env var > unauthenticated
- `gh login` validates the token via the GitHub API (`GET /user`) before storing
- `update` command uses stored token for authenticated access to private repo releases
- File permissions: `0600` on Unix (no-op on Windows)
- Uses `ureq` for GitHub API calls (same HTTP stack as `update`)
- `gh whoami` shows the authenticated user and token scopes
- `auth status` shows authentication status for all configured providers
- `auth logout` removes all stored tokens; `gh logout` removes only the GitHub token
- All commands are idempotent: login overwrites, logout succeeds if nothing stored

### `diegops devtools` — developer workstation setup
- Automates identity and tooling configuration for a fresh developer workstation
- **Identity resolution order:** `--name`/`--email` flags > `git config` values > GitHub API (via `gh` CLI)
- All subcommands are idempotent

#### `devtools git set`
- Sets `git config --global user.name` and `user.email`
- If `--name`/`--email` omitted, resolves from existing git config or GitHub API (`gh api /user`)
- Requires `git` on `$PATH`; `gh` needed only for API fallback

#### `devtools gpg init`
- Generates a GPG identity configuration file (does not create a key)
- Accepts `--name`/`--email` with same resolution order as `git set`
- Requires `gpg` on `$PATH`

#### `devtools gpg set`
- Full GPG setup: generates a GPG key, uploads public key to GitHub, configures git commit signing
- Requires `gpg` and `gh` (authenticated) on `$PATH`
- Idempotent: skips key generation if a matching key already exists

#### `devtools gpg restart`
- Restarts `gpg-agent` — fixes Windows post-restart signing errors
- Requires `gpg` on `$PATH`

#### `devtools ssh list`
- Shows local SSH keys (from `~/.ssh/`) and keys registered on GitHub (via `gh ssh-key list`)
- Requires `gh` (authenticated) for GitHub key listing

#### `devtools ssh config`
- Prints the contents of `~/.ssh/config` to stdout

#### `devtools ssh create`
- Generates a new SSH key via `ssh-keygen`
- Accepts `--name`, `--type` (key algorithm), `--email` (key comment)
- TTY detection for name conflict resolution: interactive mode prompts the user; non-interactive mode (CI/containers) exits with error
- Requires `ssh-keygen` on `$PATH`

### `diegops cadi`
- Displays the DiegOps hero screen with ASCII art branding
- No arguments, no side effects — purely informational

### `diegops update` — self-update behaviour
- Determines its own target triple at **compile time** via `#[cfg]` constants in `src/commands/update.rs`.
- Calls `GET https://api.github.com/repos/CaDi-Team/diegops-tools/releases/latest` (GitHub API).
- Finds the matching asset by suffix `<target>.tar.gz` (Unix) or `<target>.zip` (Windows).
- Downloads with `ureq` (sync, rustls TLS — no OpenSSL dependency).
- Extracts with `flate2`+`tar` (Unix) or `zip` (Windows).
- Replaces itself: atomic rename on Unix; rename-to-`.old` trick on Windows.
- Idempotent: already-up-to-date exits 0.
- Progress → **stderr**; status line → **stdout**.

## Adding a new command

1. Add a variant to `Commands` enum in `src/main.rs` with a `///` doc comment.
2. Add the match arm in `main()`.
3. If logic exceeds ~20 lines, extract to `src/commands/<name>.rs` as a public function.
4. Add integration test in `tests/integration_test.rs`.
5. Update the `## Commands` section in `README.md`.

## CI/CD

### CI (`ci.yml`)
Triggers on every push and PR to `develop`:
1. `cargo fmt --check` — formatting gate
2. `cargo clippy -- -D warnings` — lint gate
3. `cargo test` — test gate

Runner: `[cadi-hq-runner-dind-set-v2, cadi-hq-runner-dind-set]` (self-hosted Linux dind).

### Release (`cd.yml`)
Triggers on tag push matching `v[0-9]+.[0-9]+.[0-9]+`:
1. Builds all 6 targets in a matrix — all jobs on `[cadi-hq-runner-dind-set-v2, cadi-hq-runner-dind-set]`.
2. Cross-compilation via `cargo-zigbuild` (Zig as linker — no Docker, no `cross`).
3. Archives: `.tar.gz` for Unix targets (bash + `tar`), `.zip` for Windows (bash + `zip`).
4. Creates a GitHub Release with all archives as assets.

Runner: `[cadi-hq-runner-dind-set-v2, cadi-hq-runner-dind-set]` (self-hosted Linux dind) for all jobs.
**Do not** add `ubuntu-latest`, `macos-latest`, or `windows-latest` as runner values — use the self-hosted tag exclusively.

Build tools per target:
- `x86_64-unknown-linux-gnu` — native `cargo build` (runner is this target)
- All other targets — `cargo-zigbuild` (Zig pinned via `ZIG_VERSION` env var in cd.yml; Zig ships musl libc and MinGW)

**Do not** use `cross` or Docker for any target — `cargo-zigbuild` handles all cross-compilation.
**Do not** use `x86_64-pc-windows-msvc` — MSVC cross-compilation from Linux is impossible; use `x86_64-pc-windows-gnu`.

To cut a release:
```sh
git tag v1.2.3
git push origin v1.2.3
```

## Do not

- Do not use nightly Rust features or unstable APIs.
- Do not hardcode home paths (`/home/user`, `C:\Users\...`).
- Do not use `std::process::exit()` except inside `main` as a last resort.
- Do not import OS-specific crates (`winapi`, `nix`) without a `#[cfg]` guard.
- Do not commit the `target/` directory.
- Do not use `.unwrap()` outside of tests and prototypes.
- Do not add async unless a command genuinely needs concurrent I/O (clap is sync; keep it sync until needed).
- Do not break existing commands — the CLI surface is a contract.
