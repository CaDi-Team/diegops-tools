# ktool Sidecar Integration & CLI Elevation

**Date:** 2026-03-26
**Status:** Approved
**Scope:** Changes to both `diegops-tools` and `karluiz-tool-cli` repositories

## Overview

Integrate ktool as a managed sidecar binary inside diegops, and elevate ktool to match diegops' CLI maturity — adding self-update, auth management, hero screen, and version commands. Both CLIs remain fully independent; they share conventions, not code.

## Principles

- **ktool is a first-class standalone CLI.** It works without diegops. Many users will never touch diegops.
- **diegops is the orchestrator.** It can download, update, and proxy ktool — but never writes to ktool's space.
- **Convention over coupling.** Same token schema, same directory patterns, same exit code semantics — zero shared crates.
- **Idempotency everywhere.** Every command is safe to run repeatedly.

## Architecture

### Directory Layout

**ktool (standalone):**
```
~/.ktool/
├── tokens/
│   └── kenv.json          # {version, token, stored_at} — 0600 perms
└── config.toml            # app/env preferences only
```

**diegops (sidecar management):**
```
~/.diegops/
├── bin/
│   └── ktool              # managed sidecar binary
├── tokens/
│   └── gh.json            # existing GitHub token
└── ...                    # existing diegops structure
```

### Shared Conventions

| Convention | Value |
|---|---|
| Token JSON schema | `{"version": 1, "token": "...", "stored_at": "RFC3339"}` |
| File permissions | `0600` on Unix (no-op on Windows) |
| Token resolution | stored file > env var > error |
| Update source | GitHub API `GET /repos/{owner}/{repo}/releases/latest` |
| Asset naming | `{binary}-{target}.tar.gz` (Unix), `.zip` (Windows) |
| Exit codes | 0 = success, 1 = user error, 2 = internal error |
| Output discipline | data to stdout, progress/errors to stderr |

---

## ktool Changes

### New Commands

#### `ktool version`
- Prints version from `CARGO_PKG_VERSION`
- Same pattern as `diegops version`

#### `ktool update`
- Self-update from `CaDi-Team/karluiz-tool-cli` GitHub releases
- Compile-time target detection via `#[cfg]` constants
- Download with `ureq` (sync, rustls — no OpenSSL)
- Extract with `flate2` + `tar` (Unix) or `zip` (Windows)
- Atomic binary replacement: rename on Unix, rename-to-`.old` on Windows
- Token-aware: uses stored kenv token or `$GITHUB_TOKEN` for authenticated requests if available
- Idempotent: already-up-to-date exits 0

**Supported targets:**

| Platform | Target Triple |
|----------|---|
| macOS Intel | `x86_64-apple-darwin` |
| macOS Apple Silicon | `aarch64-apple-darwin` |
| Linux x64 (musl) | `x86_64-unknown-linux-musl` |
| Linux ARM64 (musl) | `aarch64-unknown-linux-musl` |

#### `ktool cadi`
- Karluiz-branded 8-bit ASCII art hero screen
- Commodore 64 aesthetic, "Developer by Passion" tagline
- Includes "Made by CaDi Labs with love <3" at the bottom
- Shows version from `CARGO_PKG_VERSION`

#### `ktool auth kenv login <TOKEN>`
- Validates token by making a test request to `GET https://karluiz.com/api/env/orbital` with Bearer token (reuses existing API endpoint — a successful auth response confirms the token is valid)
- On success, stores to `~/.ktool/tokens/kenv.json`
- Creates `~/.ktool/tokens/` directory if needed
- Sets file permissions to `0600` on Unix
- Replaces the current interactive `ktool login` command

#### `ktool auth kenv logout`
- Removes `~/.ktool/tokens/kenv.json`
- Succeeds silently if no token stored (idempotent)

#### `ktool auth kenv whoami`
- Reads stored token, calls API to show authenticated user/context
- Falls back to `$KENV_API_TOKEN` env var

#### `ktool auth status`
- Scans `~/.ktool/tokens/*.json` and shows status for each provider
- Shows token source (file vs env var) and stored timestamp

#### `ktool auth logout`
- Removes all `.json` files in `~/.ktool/tokens/`

### Modified Commands

#### `ktool kenv list` / `ktool kenv --set-app` / `ktool kenv --set-env`
- Unchanged behavior
- Token now read from `~/.ktool/tokens/kenv.json` instead of `~/.config/ktool/config.toml`
- Config (app/env) read from `~/.ktool/config.toml` instead of `~/.config/ktool/config.toml`

#### `ktool login` (removed)
- Replaced by `ktool auth kenv login <TOKEN>`
- First run migration: if `~/.config/ktool/config.toml` contains a `token` field, migrate it to `~/.ktool/tokens/kenv.json` and remove the `token` field from the old config. Move `app`/`env` fields to `~/.ktool/config.toml`.

### Token Resolution Order
1. `~/.ktool/tokens/kenv.json` (file)
2. `$KENV_API_TOKEN` environment variable
3. Error: "No token found. Run `ktool auth kenv login <TOKEN>` first."

### ktool Project Structure (after changes)

```
karluiz-tool-cli/
├── src/
│   ├── main.rs
│   ├── config.rs              # app/env config (no token)
│   ├── api.rs                 # karluiz API client
│   └── commands/
│       ├── mod.rs
│       ├── kenv.rs            # existing kenv commands
│       ├── auth.rs            # new: token management
│       ├── update.rs          # new: self-update
│       ├── cadi.rs            # new: hero screen
│       └── common.rs          # new: home_dir, ktool_dir helpers
├── tests/
│   └── integration_test.rs    # new: CLI integration tests
├── Cargo.toml
├── Cargo.lock
└── .github/workflows/
    ├── ci.yml
    └── release.yml
```

### Dependency Changes

**Add:**
- `ureq` — HTTP client for update command (sync, rustls)
- `flate2` — gzip decompression for update
- `tar` — tar extraction for update
- `zip` — zip extraction for Windows update
- `serde_json` — token JSON serialization (already has `serde_json` for API)

**Keep:**
- `reqwest` (blocking) — for karluiz API calls (already in use)
- `clap`, `serde`, `toml`, `dirs` — existing deps

**Remove:**
- `rpassword` — no longer needed (no interactive token prompt)

---

## diegops Changes

### New Command: `diegops ktool`

#### `diegops ktool update`
- Downloads latest ktool binary from `CaDi-Team/karluiz-tool-cli` releases
- Installs to `~/.diegops/bin/ktool`
- Same download logic as `diegops update` (ureq, flate2, tar/zip, atomic replace)
- Creates `~/.diegops/bin/` directory if needed
- Idempotent: already-up-to-date exits 0

#### `diegops ktool <args...>`
- All args except `update` are forwarded to `~/.diegops/bin/ktool`
- Uses `std::process::Command` with inherited stdin/stdout/stderr
- Propagates ktool's exit code
- If `~/.diegops/bin/ktool` does not exist, prints error: "ktool not installed. Run `diegops ktool update` first." and exits 1

### Enhanced Command: `diegops auth status`
- Existing behavior: shows GitHub token status from `~/.diegops/tokens/gh.json`
- New: also checks `~/.ktool/tokens/kenv.json` (read-only) and shows kenv status
- Shows "(managed by ktool)" annotation for kenv tokens
- If ktool token dir doesn't exist, shows "kenv: not configured"

### Clap Structure

```rust
#[derive(Subcommand)]
enum Commands {
    // ... existing commands ...

    /// Manage and run ktool
    Ktool {
        /// Arguments passed to ktool
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}
```

The match arm checks if `args[0] == "update"` — if so, handles internally. Otherwise, passthrough.

### diegops Project Structure (additions only)

```
src/commands/
├── ktool.rs               # new: sidecar management + passthrough
└── ...                    # existing modules unchanged
```

---

## Migration Strategy

### ktool config migration (one-time, automatic)

On any ktool command that needs auth or config:

1. Check if `~/.config/ktool/config.toml` exists with a `token` field
2. If yes:
   a. Create `~/.ktool/tokens/kenv.json` with the token (new schema)
   b. Create `~/.ktool/config.toml` with `app`/`env` fields only
   c. Remove `token` field from `~/.config/ktool/config.toml`
   d. Print migration notice to stderr
3. If no: proceed normally

Migration is idempotent — if `~/.ktool/tokens/kenv.json` already exists, skip.

---

## Testing

### ktool tests

**Unit tests:**
- Token save/load roundtrip
- Token resolution order (file > env var)
- Config save/load (app/env only, no token)
- Migration logic (old config -> new locations)
- Update target detection

**Integration tests:**
- `ktool version` exits 0, prints version
- `ktool cadi` exits 0, prints hero screen
- `ktool auth kenv login` with invalid token fails
- `ktool auth kenv logout` is idempotent
- `ktool auth status` shows providers
- `ktool --help` shows all commands
- `ktool update --help` exits 0 (no actual network call)

### diegops tests

**Integration tests:**
- `diegops ktool update --help` exits 0
- `diegops ktool` without binary installed prints error, exits 1
- `diegops auth status` works with and without ktool tokens present

---

## CI/CD

### ktool release workflow updates

Current asset naming: `ktool-linux-x86_64.tar.gz`, `ktool-macos-arm64.tar.gz`, etc.

Required asset naming for update command compatibility:
- `ktool-x86_64-apple-darwin.tar.gz`
- `ktool-aarch64-apple-darwin.tar.gz`
- `ktool-x86_64-unknown-linux-musl.tar.gz`
- `ktool-aarch64-unknown-linux-musl.tar.gz`

The release workflow must be updated to use full target triples in asset names, matching the compile-time `CURRENT_TARGET` constant.

### diegops CI

No changes needed — existing CI runs fmt + clippy + test on all pushes.
