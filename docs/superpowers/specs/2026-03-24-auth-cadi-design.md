# Auth Commands & Cadi Hero Screen — Design Spec

## Summary

Add `diegops auth` command group for credential management (starting with GitHub) and `diegops cadi` command for a retro ASCII hero screen. Update `diegops update` to use stored tokens for private repo access.

## `diegops auth` — Credential management

### Token storage

- Directory: `~/.diegops/tokens/`
- One JSON file per provider: `gh.json`
- Format: `{"version": 1, "token": "<PAT>", "stored_at": "2026-03-24T16:45:00Z"}`
- File permissions `0600` on Unix (`#[cfg(unix)]`)
- On Windows, file created with default ACLs (acceptable for single-user workstation)
- Directory created automatically on first `login`

### Token resolution (used by `update`, `whoami`, and any future token-consuming command)

Priority: `~/.diegops/tokens/gh.json` > `$GITHUB_TOKEN` env var > unauthenticated

This resolution order is consistent across all commands that need a GitHub token.

When a token is available, `update` adds `Authorization: Bearer <token>` header to all GitHub API and download requests. The API URL is the existing `RELEASES_API` constant in `update.rs` (`https://api.github.com/repos/CaDi-Team/diegops-tools/releases/latest`). No token = current behavior (works for public repos, 404 for private).

### Commands

| Command | Description |
|---------|-------------|
| `diegops auth gh login <PAT>` | Validate token via `GET /user` with auth header, store if valid |
| `diegops auth gh logout` | Remove `~/.diegops/tokens/gh.json`, confirm |
| `diegops auth gh whoami` | Show GitHub username and token scopes from stored token |
| `diegops auth status` | Show all known providers, configured/missing, stored_at timestamp |
| `diegops auth logout` | Remove all stored tokens |

### Clap enum structure

```rust
enum AuthCommand {
    Gh { cmd: GhCommand },
    Status,
    Logout,
}

enum GhCommand {
    Login { token: String },
    Logout,
    Whoami,
}
```

`auth logout` removes all providers. `auth gh logout` removes only GitHub.

### `auth gh login` flow

1. Call `GET https://api.github.com/user` with `Authorization: Bearer <PAT>`
2. If 200: extract `login` field, store token + timestamp, print to stdout: `Authenticated as <login>`
3. If 401/403: exit 1 — `error: token validation failed — check that your PAT is valid and not expired`
4. If network error: exit 2 — `error: could not reach GitHub API. Check your connection`

**Security note:** The PAT is passed as a CLI argument, which is visible in process listings. This is a known trade-off given the project's "no interactive prompts" rule. For CI, pipe via env var: `diegops auth gh login "$GH_TOKEN"`.

### `auth gh whoami` flow

1. Load token using resolution order: file > `$GITHUB_TOKEN` > none
2. If no token: exit 1 — `error: not authenticated. Run 'diegops auth gh login <PAT>' first`
3. Call `GET https://api.github.com/user` with auth header
4. Print to stdout: username, and scopes from `x-oauth-scopes` response header
5. If 401: exit 1 — `error: stored GitHub token is no longer valid. Run 'diegops auth gh login <PAT>' to update it`

### `auth status` output (stdout)

```
GitHub (gh)    [ok] configured    stored 2026-03-24T16:45:00Z
```

Or:
```
GitHub (gh)    [--] not configured
```

Uses ASCII markers `[ok]` / `[--]` for maximum terminal compatibility (Windows cmd.exe, legacy terminals).

### `auth logout` (all)

- Removes `~/.diegops/tokens/` directory contents
- Prints to stdout: count of removed tokens
- Idempotent: exits 0 if already empty

### `auth gh logout`

- Removes `~/.diegops/tokens/gh.json`
- Prints to stdout: confirmation or "not configured" if already absent
- Idempotent: exits 0

### Exit codes

All auth commands follow CLAUDE.md conventions:
- `0` — success (including idempotent "already done" states)
- `1` — user error (bad token, not authenticated, missing args)
- `2` — internal/external error (network failure, filesystem error)

### Output streams

- **stdout**: data output (status table, whoami info, confirmation messages)
- **stderr**: error messages (prefixed with `error:`)

### Error messages

| Condition | Exit | Stream | Message |
|-----------|------|--------|---------|
| Invalid PAT on login | 1 | stderr | `error: token validation failed — check that your PAT is valid and not expired` |
| Network error on login | 2 | stderr | `error: could not reach GitHub API. Check your connection` |
| 404 on update without token | 1 | stderr | `error: GitHub API returned 404. If this is a private repo, run 'diegops auth gh login <PAT>' first` |
| Stored token expired/revoked | 1 | stderr | `error: stored GitHub token is no longer valid. Run 'diegops auth gh login <PAT>' to update it` |
| Not authenticated (whoami) | 1 | stderr | `error: not authenticated. Run 'diegops auth gh login <PAT>' first` |

## `diegops cadi` — Hero screen

Prints ASCII art hero screen to stdout:

```
 ╔═══════════════════════════════════════════════════════╗
 ║                                                       ║
 ║      ___  _            ___  ___                       ║
 ║     /   \(_) ___  __ _/___\/ _ \___                   ║
 ║    / /\ /| |/ _ \/ _` //  // /_)/ __|                 ║
 ║   / /_// | |  __/ (_| / \_// ___/\__ \                 ║
 ║  /___,'  |_|\___|\__, \___/\/    |___/                 ║
 ║                  |___/                                 ║
 ║                                                       ║
 ║    "When more than one Diego is needed"                ║
 ║                                                       ║
 ║    v{VERSION} · Made by CaDi Labs with love <3         ║
 ║                                                       ║
 ╚═══════════════════════════════════════════════════════╝
```

- Version from `env!("CARGO_PKG_VERSION")`
- Output to stdout
- Uses `<3` instead of `♥` for terminal compatibility
- No dependencies, pure string constant with version interpolation

## Implementation

### Files to create/modify

| File | Change |
|------|--------|
| `src/commands/auth.rs` | New — auth command group, token storage, GitHub API validation |
| `src/commands/cadi.rs` | New — hero screen |
| `src/commands/mod.rs` | Add `pub mod auth;` and `pub mod cadi;` |
| `src/commands/update.rs` | Load token, add auth headers to `fetch_latest` and `download` |
| `src/main.rs` | Add `Auth` and `Cadi` variants to `Commands` |
| `tests/integration_test.rs` | Add help tests for auth and cadi commands |
| `README.md` | Add auth and cadi command documentation |
| `CLAUDE.md` | Add auth and cadi to Commands table and detailed sections |

### Token file I/O

- Uses `serde_json` + `serde` derive for serialization (already in Cargo.toml)
- `stored_at` uses RFC 3339 format via the `humantime` crate (`humantime::format_rfc3339`) — add as dependency
- Token loading shared between `auth` and `update` modules — put `load_gh_token()` in `auth.rs` as a public function

### HTTP with auth

`ureq` already supports `.set("Authorization", ...)`. The `update.rs` functions `fetch_latest` and `download` gain an optional token parameter. No new HTTP dependencies.

## Testing

- Integration tests: `auth --help`, `auth gh --help`, `auth status --help`, `cadi --help`, `cadi` (full output)
- Unit tests: token file serialization/deserialization, RFC 3339 timestamp formatting, `load_gh_token` with missing/present file
- `auth gh login`, `whoami`, and `update` with token are not testable offline — test only `--help` flags
- `cadi` command can be fully tested (no network, deterministic output)
