# Devtools Commands — Design Spec

## Summary

Add `diegops devtools` command group with three subgroups: `gpg` (GPG key management and git signing setup), `ssh` (SSH key management), and `git` (git identity setup). All commands shell out to system tools (`gpg`, `gpgconf`, `ssh-keygen`, `gh`, `git`) and provide clear error messages when tools are missing or fail.

## TTY detection

All `devtools` commands use `std::io::IsTerminal` on stdin to detect interactive usage:
- **TTY**: prompt user for input when needed (e.g., SSH key name postfix on conflict)
- **No TTY**: use auto-generated defaults, no prompts

## `diegops devtools git`

### Commands

| Command | Description |
|---------|-------------|
| `diegops devtools git set [--name NAME] [--email EMAIL]` | Set git global `user.name` and `user.email` |

### `git set` flow

1. Resolve name: `--name` flag > `gh api user` `.login` field
2. Resolve email: `--email` flag > `gh api user/emails` primary email
3. If `git config --global user.name` already matches, skip (print "already set")
4. If `git config --global user.email` already matches, skip (print "already set")
5. Run `git config --global user.name <name>` and `git config --global user.email <email>`
6. Print summary to stdout

### Error messages

| Condition | Message |
|-----------|---------|
| `git` not on PATH | `error: git not found on PATH. Install it from https://git-scm.com/` |
| `gh` not on PATH (when no flags) | `error: gh CLI not found — provide --name and --email flags, or install gh from https://cli.github.com/` |
| `gh` not authenticated | `error: gh is not authenticated. Run 'gh auth login' or provide --name and --email flags` |
| API fails | `error: could not fetch identity from GitHub API: <details>` |

## `diegops devtools gpg`

### Commands

| Command | Description |
|---------|-------------|
| `diegops devtools gpg init [--name NAME] [--email EMAIL]` | Generate `~/.diegops/gpg-config.yaml` with identity |
| `diegops devtools gpg set` | Generate GPG key, upload to GitHub, configure git signing |
| `diegops devtools gpg restart` | Kill and relaunch `gpg-agent` |

### `gpg init` flow

1. Resolve identity:
   - `--name`/`--email` flags (highest priority)
   - `git config --global user.name` / `user.email`
   - `gh api user` + `gh api user/emails`
2. Write `~/.diegops/gpg-config.yaml`:
   ```yaml
   name: "Diego Pinto"
   email: "diegopintog@outlook.com"
   ```
3. Idempotent: if file exists, print location and exit 0
4. Print summary to stdout

### `gpg set` flow

1. Pre-flight: check `gpg`, `git`, `gh` on PATH
2. Load config from `~/.diegops/gpg-config.yaml` — error if missing (tell user to run `gpg init` first)
3. Check for existing GPG secret key matching email (`gpg --list-secret-keys --keyid-format=long <email>`)
   - If found: use it, skip generation
4. If not found: generate 4096-bit RSA key (no passphrase, no expiry) via `gpg --batch --gen-key`
5. Export public key (`gpg --armor --export <key-id>`)
6. Check if key already on GitHub (`gh api user/gpg_keys`)
   - If found: skip upload
7. If not found: upload via `gh api user/gpg_keys --method POST --field armored_public_key=<key>`
8. Set git config:
   - `user.signingkey = <key-id>`
   - `commit.gpgsign = true`
   - `tag.gpgsign = true`
   - `gpg.program = <path-to-gpg>`
9. Print summary to stdout

### `gpg restart` flow

1. Check `gpgconf` on PATH
2. Run `gpgconf --kill gpg-agent`
3. Run `gpgconf --launch gpg-agent`
4. Verify with `gpg-connect-agent /bye`
5. Print status to stdout

### Error messages

| Condition | Message |
|-----------|---------|
| `gpg` not on PATH | `error: gpg not found on PATH. Install it: gpg4win (Windows), brew install gnupg (macOS), apt install gnupg (Linux)` |
| `gpgconf` not on PATH | `error: gpgconf not found on PATH. It usually ships with gpg — reinstall your GPG package` |
| `gh` not on PATH | `error: gh CLI not found on PATH. Install it from https://cli.github.com/` |
| `gh` not authenticated | `error: gh is not authenticated. Run 'gh auth login' first` |
| Config missing for `set` | `error: gpg config not found. Run 'diegops devtools gpg init' first` |
| Key generation failed | `error: GPG key generation failed: <gpg stderr>` |
| Upload failed (scope) | `error: failed to upload GPG key to GitHub. Ensure your token has the 'write:gpg_key' scope` |
| `gpg-agent` restart failed | `error: could not restart gpg-agent: <details>` |

## `diegops devtools ssh`

### Commands

| Command | Description |
|---------|-------------|
| `diegops devtools ssh list` | Show local keys + GitHub registered keys |
| `diegops devtools ssh config` | Print `~/.ssh/config` contents |
| `diegops devtools ssh create [--name NAME] [--type TYPE] [--email EMAIL]` | Create a new SSH key |

### `ssh list` output

```
Local keys (~/.ssh/):
  NAME                 TYPE      FINGERPRINT                              PUBLIC KEY
  id_ed25519           ed25519   SHA256:abc123...                         ssh-ed25519 AAAA... user@host
  id_rsa               rsa       SHA256:def456...                         ssh-rsa AAAA... user@host

GitHub registered keys:
  TITLE                TYPE      FINGERPRINT                              ADDED
  laptop               ed25519   SHA256:abc123...                         2026-01-15
  ci-deploy            ed25519   SHA256:xyz789...                         2025-11-20
```

Flow:
1. Scan `~/.ssh/` for files matching `*.pub`
2. For each `.pub` file, run `ssh-keygen -l -f <file>` to get fingerprint and type
3. Read `.pub` content for public key display
4. Call `gh ssh-key list` for GitHub registered keys (graceful if `gh` not available)
5. Print both sections

### `ssh config` flow

1. Read `~/.ssh/config`
2. If exists: print contents to stdout
3. If not exists: print "No SSH config found at ~/.ssh/config"

### `ssh create` flow

1. Resolve type: `--type` flag > default `ed25519`
2. Resolve name: `--name` flag > default `id_<type>`
3. Resolve email: `--email` flag > `git config user.email` > `gh api user/emails`
4. Target path: `~/.ssh/<name>`
5. If target path exists:
   - TTY: prompt user for a postfix string, new path becomes `~/.ssh/<name>_<postfix>`
   - No TTY: generate random 6-char alphanumeric postfix
6. Run `ssh-keygen -t <type> -C <email> -f <path> -N ""` (empty passphrase)
7. Print the public key to stdout for easy copy-paste
8. Print path to stdout

### Error messages

| Condition | Message |
|-----------|---------|
| `ssh-keygen` not on PATH | `error: ssh-keygen not found on PATH. Install OpenSSH` |
| Key generation failed | `error: ssh-keygen failed: <stderr>` |
| `~/.ssh/` not readable | `error: could not read ~/.ssh/ directory: <details>` |
| `gh` not available for list | Shows local keys only, prints note: `(GitHub keys unavailable — gh CLI not found or not authenticated)` |

## Exit codes

All devtools commands follow CLAUDE.md conventions:
- `0` — success (including idempotent "already done" states)
- `1` — user error (missing config, bad flags)
- `2` — internal/external error (tool not found, network failure)

## Output streams

- **stdout**: data output (summaries, key listings, config contents, public keys)
- **stderr**: progress messages and errors

## Implementation

### Files

| File | Change |
|------|--------|
| `src/commands/devtools.rs` | New — devtools command group with clap enum, dispatches to submodules |
| `src/commands/devtools_git.rs` | New — git set command |
| `src/commands/devtools_gpg.rs` | New — GPG init/set/restart |
| `src/commands/devtools_ssh.rs` | New — SSH list/config/create |
| `src/commands/mod.rs` | Add `pub mod devtools;` etc. |
| `src/main.rs` | Add `Devtools` variant |
| `tests/integration_test.rs` | Help tests for all devtools subcommands |
| `README.md` | Document devtools commands |
| `CLAUDE.md` | Add devtools to Commands table and descriptions |

### Shared patterns

- Identity resolution (name + email from flags > git config > gh api) is used by both `git set` and `gpg init` — extract to a shared helper in `devtools.rs`
- All commands shell out via `std::process::Command`
- TTY detection via `std::io::IsTerminal`

### Dependencies

No new crate dependencies. `rand` is NOT needed — generate random postfix from `/dev/urandom` or `getrandom` syscall via a small inline function, or simply use a timestamp-based suffix.

## Testing

- Integration tests: `--help` for all subcommands
- Unit tests: identity resolution logic, SSH key file scanning, config YAML generation
- Commands that shell out to `gpg`/`ssh-keygen`/`gh` are not testable offline — test only `--help` and pure logic
