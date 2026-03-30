# `diegops secrets` — Vault-backed workstation file sync

**Date:** 2026-03-28
**Status:** Approved

## Overview

A new `diegops secrets` command that syncs sensitive workstation files (SSH keys, kubeconfigs, and others) through HashiCorp Vault KV v2. Files are base64-encoded in Vault and restored with correct permissions on pull. This keeps secrets out of git while enabling reproducible workstation setup across machines.

## Commands

| Command | Description |
|---------|-------------|
| `diegops secrets init` | Create sample `~/.diegops/secrets.yaml` |
| `diegops secrets push [--path P] [--config F]` | Local files → Vault (base64-encoded) |
| `diegops secrets pull [--path P] [--config F]` | Vault → local files (decoded, permissions set) |
| `diegops secrets status [--config F]` | Compare local vs Vault, show sync state |

- `--path` filters to folders whose expanded `dest` starts with the given prefix (same pattern as `vault apply --path`).
- `--config` overrides the config file path.

## Config

**File:** `~/.diegops/secrets.yaml`
**Env var override:** `DIEGOPS_SECRETS_CONFIG`
**Resolution order:** `--config` flag > `DIEGOPS_SECRETS_CONFIG` > `~/.diegops/secrets.yaml`

### Format

```yaml
folders:
  - dest: $HOME/.ssh
    vault_path: secret/workstation/ssh
    keys:
      - id_ed25519
      - id_ed25519.pub
      - config
    dir_mode: "0700"    # optional, default 0700

  - dest: $HOME/.kube
    vault_path: secret/workstation/kube
    keys: "*"           # pull all keys from this vault path
```

### Key selectors

The `keys` field accepts:

- **A list of strings** — explicit filenames to sync
- **A list of objects** — filenames with per-file permission overrides
- **The string `"*"`** — sync all keys from the Vault path

Mixed lists are supported:

```yaml
keys:
  - name: id_ed25519
    mode: "0600"
  - id_ed25519.pub       # uses smart default (0644)
```

Each key entry is either:
- A plain string (filename, uses smart default permissions)
- An object with `name` (required) and `mode` (optional)

### Serde model

```rust
#[derive(Deserialize)]
struct SecretsConfig {
    folders: Vec<Folder>,
}

#[derive(Deserialize)]
struct Folder {
    dest: String,
    vault_path: String,
    keys: KeySelector,       // reuse the untagged enum pattern
    dir_mode: Option<String>, // e.g. "0700", default 0700
}

#[derive(Deserialize)]
#[serde(untagged)]
enum KeySelector {
    All(String),              // "*"
    List(Vec<KeyEntry>),
}

#[derive(Deserialize)]
#[serde(untagged)]
enum KeyEntry {
    Simple(String),           // just filename
    WithMode { name: String, mode: Option<String> },
}
```

## Vault storage convention

- Each folder maps to **one Vault KV v2 secret**.
- Keys = filenames, values = base64-encoded file content.
- Example: `secret/workstation/ssh` contains keys `id_ed25519`, `id_ed25519.pub`, `config`.

Storing manually (for reference, diegops push handles this):
```bash
vault kv put secret/workstation/ssh \
  id_ed25519="$(base64 -w0 < ~/.ssh/id_ed25519)" \
  id_ed25519.pub="$(base64 -w0 < ~/.ssh/id_ed25519.pub)"
```

## Permission model

### Smart defaults

| Pattern | Default mode |
|---------|-------------|
| Private keys: `id_*` without `.pub` extension | `0600` |
| Public keys: `*.pub` | `0644` |
| Everything else | `0600` |
| Directories | `0700` (overridable via `dir_mode`) |

### Override

Per-file `mode` in config takes precedence over smart defaults.

### Implementation

```rust
fn default_mode(filename: &str) -> u32 {
    if filename.ends_with(".pub") {
        0o644
    } else {
        0o600
    }
}
```

Directories use `dir_mode` from config, defaulting to `0o700`.

## Push behavior

1. **Pre-flight checks** (fatal on failure):
   - vault binary on PATH
   - `VAULT_ADDR` set
   - vault authenticated (`vault token lookup`)

2. **For each folder in config:**
   - Expand `dest` path (`$HOME` substitution)
   - If dest directory doesn't exist: fail this folder, continue
   - Resolve file list:
     - Explicit `keys`: read listed files from dest
     - `keys: "*"`: read all regular files in dest (non-recursive, skip subdirectories and dotfiles like `.DS_Store`)
   - Base64-encode each file's content (no line wrapping)
   - Fetch current Vault content, compare with local
   - If all values match: skip (already in sync)
   - Otherwise: `vault kv put <vault_path> file1=<b64> file2=<b64> ...`

3. **Report:** `N pushed, N unchanged, N failed`

### Push with `keys: "*"` on first run

On the first machine, `keys: "*"` reads all files in the dest directory. This bootstraps Vault from the existing local state. On subsequent machines, `pull` with `keys: "*"` fetches whatever is stored in Vault.

## Pull behavior

1. **Pre-flight checks** (same as push)

2. **For each folder in config:**
   - Expand `dest` path
   - Create dest directory if missing (with `dir_mode`, default `0700`)
   - `vault kv get -format=json <vault_path>`
   - Resolve file list:
     - Explicit `keys`: pull only listed keys
     - `keys: "*"`: pull all keys from Vault response
   - For each key:
     - Base64-decode the value
     - Determine file mode (per-file override > smart default)
     - Compare with local file if it exists:
       - **Matches:** skip
       - **Differs:** back up existing to `<filename>.bak`, write new file, set permissions
       - **Not found in Vault but listed:** fail this key
     - If local file doesn't exist: write, set permissions

3. **Report:** `N written, N unchanged, N backed up, N failed`

### Backup behavior

- Backup file: `<filename>.bak` (e.g. `id_ed25519.bak`)
- Only one `.bak` kept — previous backup is overwritten
- `.bak` files inherit the same permissions as the original
- `.bak` files are never pushed to Vault

## Status behavior

For each folder, compare local files against Vault:

| Status | Meaning |
|--------|---------|
| `SYNCED` | Local file matches Vault content |
| `LOCAL_ONLY` | File exists locally but not in Vault |
| `VAULT_ONLY` | File exists in Vault but not locally |
| `DIFFERS` | Both exist but content differs |
| `MISSING` | Dest directory doesn't exist |

Output format:
```
$HOME/.ssh
  SYNCED      id_ed25519
  SYNCED      id_ed25519.pub
  LOCAL_ONLY  known_hosts

$HOME/.kube
  DIFFERS     config
  VAULT_ONLY  config-staging
```

## Init behavior

Creates `~/.diegops/secrets.yaml` with sample content. Idempotent — if file exists, prints location and exits 0.

### Sample config

```yaml
# diegops secrets configuration
#
# Each folder maps a local directory to a Vault KV v2 path.
# Files are stored as base64-encoded values in Vault.
#
# Commands:
#   diegops secrets push    — upload local files to Vault
#   diegops secrets pull    — download files from Vault
#   diegops secrets status  — compare local vs Vault
#
# Path rules:
#   - Use $HOME as a portable prefix (works on Linux, macOS, and WSL).
#   - vault_path uses the logical Vault path (no /data/ segment).
#   - keys: list specific filenames, or use "*" to sync all files.
#   - The vault CLI must be installed and authenticated.

folders:

  - dest: $HOME/.ssh
    vault_path: secret/workstation/ssh
    keys:
      - id_ed25519
      - id_ed25519.pub
    dir_mode: "0700"

  - dest: $HOME/.kube
    vault_path: secret/workstation/kube
    keys: "*"
```

## File structure

New file: `src/commands/secrets.rs`

Reuses from existing code:
- `super::common::resolve_config_path` — config resolution
- `super::common::read_config_file` — config loading
- `super::common::expand_home` — `$HOME` expansion
- `super::common::diegops_dir` — default config directory
- Pre-flight checks pattern from `vault.rs` (`check_vault_binary`, `check_vault_addr`, `check_vault_auth`)

The vault pre-flight check functions should be extracted to a shared location (either `common.rs` or a new `vault_helpers.rs`) to avoid duplication.

## Integration with main.rs

Add `Secrets` variant to the `Commands` enum with `SecretsCommand` subcommand:

```rust
/// Sync workstation secrets via Vault
Secrets {
    #[command(subcommand)]
    command: secrets::SecretsCommand,
},
```

## Error handling

- Pre-flight failures are fatal (exit immediately with clear message)
- Per-folder failures are collected and reported at the end (same pattern as `vault apply`)
- Base64 decode failures: report the key name and vault path, skip the file, continue
- Missing keys in Vault (when using explicit list): fail that key, report, continue

## Testing

### Unit tests (inline `#[cfg(test)]`)
- Config parsing: explicit keys, wildcard, mixed entries with mode overrides
- `default_mode()`: private key, public key, generic file
- Base64 round-trip: encode local → decode matches original
- Status comparison logic

### Integration tests (`tests/`)
- `diegops secrets init` creates sample config, is idempotent
- `diegops secrets --help` shows subcommands

### Not tested (requires live Vault)
- Push/pull with real Vault — manual testing only

## Windows considerations

- Permission model (`0600`, `0700`) only applies on Unix via `#[cfg(unix)]`
- On Windows: files are written without explicit permission setting
- Base64 encoding/decoding is platform-independent
- `$HOME` expansion uses `std::env::var("HOME")` which works on WSL; native Windows uses `USERPROFILE` (handled by `common::expand_home`)
