# CLAUDE.md — diegops

## Project overview

`diegops` is a personal productivity CLI written in **Rust**, designed for first-class use across:

| Platform | Targets |
|----------|---------|
| macOS Apple Silicon | `aarch64-apple-darwin` |
| macOS Intel | `x86_64-apple-darwin` |
| Ubuntu / Debian | `x86_64-unknown-linux-gnu` |
| Alpine (containers) | `x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl` |
| Windows (WSL + native terminal) | `x86_64-pc-windows-gnu` |

Both interactive human use and container/CI workflow automation are first-class use cases. The binary must run standalone with no external runtime.

## Stack

- **Language**: Rust (stable channel — do not use nightly features)
- **CLI framework**: `clap` v4 with derive macros
- **HTTP**: `ureq` v2 (sync, rustls TLS — no OpenSSL, no C dependencies)
- **Build**: Cargo
- **CI**: GitHub Actions (`cargo fmt`, `cargo clippy`, `cargo test`)
- **Releases**: GitHub Actions on `vx.y.z` tags → GitHub Release artifacts
- **Cross-compilation**: `cargo-zigbuild` (Zig as linker — no Docker, no `cross`)
- **Runner**: `ubuntu-latest` (self-hosted Linux dind)

## Repository structure

```
diegops-tools/
├── src/
│   ├── main.rs                # CLI entry point and command dispatch
│   └── commands/
│       ├── mod.rs             # Module declarations
│       ├── auth.rs            # Token management (GitHub + ktool kenv read)
│       ├── cadi.rs            # Hero screen
│       ├── common.rs          # Shared helpers (home_dir, diegops_dir, etc.)
│       ├── devtools.rs        # Devtools clap definitions
│       ├── devtools_git.rs    # git identity setup
│       ├── devtools_gpg.rs    # GPG key management
│       ├── devtools_ssh.rs    # SSH key management
│       ├── ktool.rs           # ktool sidecar (download + passthrough)
│       ├── repo.rs            # Repository workspace management
│       ├── sync.rs            # Cloud config backup via GitHub API
│       ├── tool.rs            # Managed DevOps CLI toolbox
│       ├── update.rs          # Self-update from GitHub Releases
│       └── vault.rs           # Vault secret injection
├── tests/
│   └── integration_test.rs    # End-to-end CLI tests via process::Command
├── Cargo.toml
├── Cargo.lock                 # Always committed — this is a binary crate
└── .github/
    └── workflows/
        ├── ci.yml             # PR gate: fmt + clippy + test
        └── cd.yml             # Release: builds all targets on vx.y.z tag push
```

## Core principles

### Idempotency
Every command must be safe to run multiple times with identical results. Never produce an error on a second run for state that already exists. Prefer upsert semantics over create-only.

### Cross-platform compatibility
- **Paths**: always use `std::path::PathBuf` / `Path`. Never concatenate strings with `/` or `\`.
- **Env vars**: use `std::env::var("NAME")` — never assume POSIX or Windows env conventions.
- **No shell assumptions**: do not call `sh`, `bash`, `cmd.exe`, or `powershell` implicitly.
- **Platform guards**: any OS-specific code requires a `#[cfg(target_os = "...")]` attribute.
- **Terminal output**: use `println!` / `eprintln!`. Avoid raw ANSI escape codes unless guarded.
- **Static linking for musl**: Alpine/container builds use musl targets (static binary). Ensure no dynamic glibc dependencies.

### Exit codes
- `0` — success
- `1` — user error (bad arguments, file not found, etc.)
- `2` — internal / unexpected error

Never call `std::process::exit()` outside `main`. Propagate errors with `?`. Exception: tool passthrough propagates child exit codes.

### stdout vs stderr
- **stdout** (`println!`) — command output and data
- **stderr** (`eprintln!`) — errors, warnings, and progress messages

### TTY / no-TTY awareness
The binary must work correctly without a TTY (containers, CI, piped output):
- Never emit interactive prompts. Use flags/arguments instead.
- Do not assume stdin is a terminal.

### Code quality gates

```sh
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

- No `.unwrap()` in production code — use `?` or explicit error handling
- `main()` returns `Result<(), Box<dyn std::error::Error>>`
- All public items must have doc comments (`///`)
- Keep `main()` as a thin dispatcher; extract logic to `src/commands/<name>.rs`

### Testing
- **Unit tests**: `#[cfg(test)]` blocks inline with the code
- **Integration tests**: `tests/` directory, using `std::process::Command`
- Tests must run **offline** — no network calls
- Use `env!("CARGO_BIN_EXE_diegops")` in integration tests

## Commands

| Command | Description |
|---------|-------------|
| `diegops version` | Print version string |
| `diegops update` | Self-update from GitHub Releases |
| `diegops cadi` | Show the DiegOps hero screen |
| `diegops bootstrap` | Full workstation setup in one shot |
| `diegops shell init` | Set up zsh, oh-my-zsh, plugins, and managed config |
| `diegops repo init` | Create sample config at `~/.diegops/repos.yaml` |
| `diegops repo apply [--path P] [--config F]` | Clone all missing repos (idempotent) |
| `diegops repo list [--config F]` | Show cloned repos |
| `diegops repo list-diff [--config F]` | Show repos not yet cloned |
| `diegops vault init` | Create sample config at `~/.diegops/repo-vault.yaml` |
| `diegops vault apply [--path P] [--config F]` | Pull secrets and write `.env` files |
| `diegops vault list [--config F]` | Show targets with `.env` |
| `diegops vault list-diff [--config F]` | Show targets missing `.env` |
| `diegops secrets init` | Create sample secrets config |
| `diegops secrets push [--path P] [--config F]` | Upload local files to Vault |
| `diegops secrets pull [--path P] [--config F]` | Download files from Vault |
| `diegops secrets status [--config F]` | Compare local vs Vault |
| `diegops auth gh login <PAT>` | Validate and store GitHub token |
| `diegops auth gh logout` | Remove GitHub token |
| `diegops auth gh whoami` | Show authenticated GitHub user |
| `diegops auth status` | Show auth status (GitHub + kenv) |
| `diegops auth logout` | Remove all tokens |
| `diegops sync push` | Upload configs to GitHub |
| `diegops sync pull` | Download configs from GitHub |
| `diegops sync status` | Show local vs cloud diff |
| `diegops tool list` | Show available tools + install status |
| `diegops tool install <name>` | Install a DevOps CLI tool |
| `diegops tool update [name]` | Update installed tools |
| `diegops tool remove <name>` | Remove a tool |
| `diegops <tool> <args>` | Passthrough to managed tool |
| `diegops ktool update` | Download/update ktool sidecar |
| `diegops ktool <args>` | Forward to managed ktool |
| `diegops devtools git set [--name] [--email]` | Set git identity |
| `diegops devtools gpg init [--name] [--email]` | Generate GPG identity config |
| `diegops devtools gpg set` | Full GPG setup |
| `diegops devtools gpg restart` | Restart gpg-agent |
| `diegops devtools ssh list` | List SSH keys |
| `diegops devtools ssh config` | Print SSH config |
| `diegops devtools ssh create [--name] [--type] [--email]` | Create SSH key |

### `diegops tool` — managed DevOps toolbox

Static registry of 10 tools compiled into the binary:

| Tool | Source | Description |
|------|--------|-------------|
| gh | `cli/cli` (GitHub) | GitHub CLI |
| vault | `hashicorp/vault` (GitHub) | HashiCorp Vault |
| terraform | `hashicorp/terraform` (GitHub) | Infrastructure as code |
| helm | `helm/helm` (GitHub) | Kubernetes package manager |
| k9s | `derailed/k9s` (GitHub) | Kubernetes TUI |
| kubectl | `dl.k8s.io` (CDN) | Kubernetes CLI |
| jq | `jqlang/jq` (GitHub) | JSON processor |
| yq | `mikefarah/yq` (GitHub) | YAML processor |
| trivy | `aquasecurity/trivy` (GitHub) | Security scanner |
| trippy | `fujiapple852/trippy` (GitHub) | Network diagnostic |

- Binaries installed to `~/.diegops/bin/`
- `diegops <tool> <args>` passthrough via `external_subcommand` in clap
- Version detection: run `--version` or `version`, extract semver pattern
- GitHub token used for API rate limit headroom (all repos are public)

### `diegops ktool` — karluiz tools sidecar

- Binary managed at `~/.diegops/bin/ktool`
- `update` downloads from `CaDi-Team/karluiz-tool-cli` GitHub Releases
- All other args forwarded to the managed binary
- Version comparison normalizes `v` prefix (tag `v0.2.4` vs output `0.2.4`)

### `diegops secrets` — workstation file sync via Vault

Config: `~/.diegops/secrets.yaml` (override with `--config` or `$DIEGOPS_SECRETS_CONFIG`)

Syncs sensitive workstation files (SSH keys, kubeconfigs, etc.) through Vault KV v2.
Files are stored as base64-encoded values. Push reads local files and writes to Vault.
Pull reads from Vault and writes locally with correct permissions.

- Push: `diegops secrets push` — base64-encodes local files and writes to Vault
- Pull: `diegops secrets pull` — decodes from Vault and writes with permissions (backs up existing files to `.bak`)
- Status: `diegops secrets status` — shows SYNCED / DIFFERS / LOCAL_ONLY / VAULT_ONLY per file
- Smart permissions: `*.pub` files get `0644`, everything else `0600`, directories `0700`
- Per-file mode overrides supported in config

### `diegops sync` — cloud config backup

- Repo: `diegops-{gh_username}-memory` (private, auto-created on first push)
- Syncs `~/.diegops/` excluding `tokens/` and `bin/`
- Uses GitHub Contents API (`GET`/`PUT /repos/{owner}/{repo}/contents/{path}`)
- Content comparison via GitHub blob SHA: `SHA1("blob {size}\0{content}")`
- `push` overwrites cloud, `pull` overwrites local — explicit intent, no conflict prompts

### `diegops auth` — credential management

- Token storage: `~/.diegops/tokens/gh.json` (`{version, token, stored_at}`)
- Resolution: stored file > `$GITHUB_TOKEN` > unauthenticated
- `auth status` also reads `~/.ktool/tokens/kenv.json` (read-only)
- File permissions: `0600` on Unix
- Private repo downloads use asset API URL with `Accept: application/octet-stream` (not `browser_download_url` which 404s for private repos)

### `diegops update` — self-update

- Compile-time target detection via `#[cfg]` constants
- Downloads from `CaDi-Team/diegops-tools` GitHub Releases
- Atomic binary replacement (rename on Unix, rename-to-`.old` on Windows)
- Permission denied detection: suggests `sudo diegops update`

## Adding a new command

1. Add a variant to `Commands` enum in `src/main.rs`
2. Add the match arm in `main()`
3. Extract logic to `src/commands/<name>.rs`
4. Add integration test in `tests/integration_test.rs`
5. Update `README.md` and the command table above

## Adding a new managed tool

1. Add a `ToolDef` entry to the `TOOLS` array in `src/commands/tool.rs`
2. Add asset naming logic in `asset_name()` for the new tool
3. Add unit tests for the new tool's asset URL generation
4. Update `README.md` tool table

## CI/CD

### CI (`ci.yml`)
Triggers on every push and PR to `develop`:
1. `cargo fmt --check`
2. `cargo clippy -- -D warnings`
3. `cargo test`

Runner: `ubuntu-latest`

### Release (`cd.yml`)
Triggers on tag push matching `v[0-9]+.[0-9]+.[0-9]+`:
1. Builds all 6 targets in a matrix
2. Cross-compilation via `cargo-zigbuild`
3. Archives: `.tar.gz` (Unix), `.zip` (Windows)
4. Creates GitHub Release with all archives

Runner: `ubuntu-latest`

**Do not** use `ubuntu-latest`, `macos-latest`, or `windows-latest`.
**Do not** use `cross` or Docker — `cargo-zigbuild` handles all cross-compilation.
**Do not** use `x86_64-pc-windows-msvc` — use `x86_64-pc-windows-gnu`.

## Do not

- Do not use nightly Rust features or unstable APIs
- Do not hardcode home paths (`/home/user`, `C:\Users\...`)
- Do not use `std::process::exit()` except in `main` (exception: tool passthrough exit code propagation)
- Do not import OS-specific crates without `#[cfg]` guard
- Do not commit `target/`
- Do not use `.unwrap()` outside tests
- Do not add async unless genuinely needed
- Do not break existing commands — the CLI surface is a contract
