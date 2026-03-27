# Cloud Sync — Design Spec

**Date:** 2026-03-27
**Status:** Approved

## Overview

Add `diegops sync push/pull/status` commands to back up and restore `~/.diegops/` config files to a private GitHub repo, using the GitHub REST API.

## Commands

| Command | Description |
|---------|-------------|
| `diegops sync push` | Upload local configs to GitHub (auto-creates repo on first use) |
| `diegops sync pull` | Download configs from GitHub to local `~/.diegops/` |
| `diegops sync status` | Show diff between local and cloud |

## Sync Scope

**Included:** All files in `~/.diegops/` recursively.

**Excluded:**
- `tokens/` directory — sensitive credentials, machine-specific
- `bin/` directory — managed binaries (ktool), large and platform-specific

Files are stored at repo root with their relative path from `~/.diegops/`. For example, `~/.diegops/repos.yaml` becomes `repos.yaml` in the repo, and `~/.diegops/some/nested/file.yaml` becomes `some/nested/file.yaml`.

## Repository

- **Name:** `diegops-{gh_username}-memory`
- **Owner:** user's personal GitHub account (always)
- **Visibility:** private
- **Creation:** automatic on first `push` via `POST /user/repos`
- **No README, no license, no .gitignore** — pure config storage

## Push Behavior

1. Load GitHub token via `auth::load_gh_token()` — error if missing
2. Get authenticated username via `GET /user`
3. Check if repo `{user}/diegops-{user}-memory` exists (`GET /repos/{owner}/{repo}`)
4. If not found (404), create it: `POST /user/repos` with `{name, private: true}`
5. Scan `~/.diegops/` recursively for files, skip `tokens/` and `bin/` directories
6. For each file:
   a. Read local content, base64-encode it
   b. Try `GET /repos/{owner}/{repo}/contents/{path}` to get current SHA (if file exists in cloud)
   c. `PUT /repos/{owner}/{repo}/contents/{path}` with `{message, content, sha?}` — creates or updates
   d. Skip if local content matches cloud content (compare SHA)
7. Progress per file to stderr: `SKIP repos.yaml (unchanged)`, `PUSH repos.yaml`, `PUSH repo-vault.yaml`
8. Summary to stdout: `Pushed 2 file(s), skipped 1.`

## Pull Behavior

1. Load GitHub token — error if missing
2. Get username, check repo exists
3. If repo doesn't exist: error "No sync repo found. Run `diegops sync push` first."
4. List repo contents recursively via `GET /repos/{owner}/{repo}/contents/{path}`
5. For each file:
   a. Fetch content (base64 in API response), decode
   b. Write to `~/.diegops/{path}`, creating parent directories as needed
   c. Skip if local content already matches (compare SHA)
   d. Set 0600 permissions on Unix for YAML files containing secrets
6. Progress per file to stderr: `SKIP repos.yaml (unchanged)`, `PULL repos.yaml`
7. Summary to stdout: `Pulled 2 file(s), skipped 1.`

## Status Behavior

1. Load token, get username, check repo exists
2. If repo doesn't exist: print "No sync repo found." and exit 0
3. Fetch cloud file list with SHAs via `GET /repos/{owner}/{repo}/contents/`
4. Scan local `~/.diegops/` (excluding tokens/, bin/)
5. Compare using GitHub's blob SHA format: `SHA1("blob {size}\0{content}")`
6. Display:
   - Files matching: `  repos.yaml (in sync)`
   - Files differing: `* repo-vault.yaml (local differs from cloud)`
   - Files only local: `+ new-config.yaml (local only)`
   - Files only cloud: `- old-config.yaml (cloud only)`

## GitHub API Endpoints Used

| Endpoint | Purpose |
|----------|---------|
| `GET /user` | Get authenticated username |
| `GET /repos/{owner}/{repo}` | Check if sync repo exists |
| `POST /user/repos` | Create sync repo (private) |
| `GET /repos/{owner}/{repo}/contents/{path}` | Get file content + SHA from cloud |
| `PUT /repos/{owner}/{repo}/contents/{path}` | Create or update a file |

All requests use:
- `Authorization: Bearer {token}`
- `User-Agent: diegops/{version}`
- `Accept: application/vnd.github.v3+json`

## Error Handling

| Condition | Behavior |
|-----------|----------|
| No token | Exit 1: "Not authenticated. Run 'diegops auth gh login <PAT>' first." |
| No network | Exit 2: "Could not reach GitHub API. Check your connection." |
| Repo missing on pull | Exit 1: "No sync repo found. Run 'diegops sync push' first." |
| 401/403 on API | Exit 1: "GitHub token invalid or lacks permissions. Run 'diegops auth gh login <PAT>'." |
| File read/write error | Print warning to stderr, continue with remaining files |

## Token Scope Requirements

The GitHub PAT must have:
- `repo` scope (to create private repos and read/write contents)

This is the same scope needed for `diegops update` on private repos.

## Project Structure

```
src/commands/
├── sync.rs              # new: sync push/pull/status logic
└── ...                  # existing modules unchanged
```

Clap structure:
```rust
/// Sync config files to GitHub
Sync {
    #[command(subcommand)]
    cmd: SyncCommand,
}

enum SyncCommand {
    Push,
    Pull,
    Status,
}
```

## Testing

**Unit tests (in sync.rs):**
- GitHub blob SHA computation matches known values
- File scanning excludes tokens/ and bin/
- Repo name generation from username

**Integration tests:**
- `diegops sync --help` exits 0, shows push/pull/status
- `diegops sync push --help` exits 0
- `diegops sync pull --help` exits 0
- `diegops sync status --help` exits 0

No network tests — sync commands require live GitHub access.

## Idempotency

- `push` when cloud matches local: skips all files, prints "Already in sync."
- `pull` when local matches cloud: skips all files, prints "Already in sync."
- `push` auto-creates repo only if missing — subsequent pushes reuse it
- All commands safe to run repeatedly
