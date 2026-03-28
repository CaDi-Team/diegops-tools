# diegops

[![CI](https://github.com/CaDi-Team/diegops-tools/actions/workflows/ci.yml/badge.svg)](https://github.com/CaDi-Team/diegops-tools/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

Personal productivity CLI for humans and containers — workspace management, secret injection, cloud config sync, managed DevOps toolbox, and developer workstation setup in a single static binary.

---

## Supported Platforms

| OS | Architecture | Target |
|----|-------------|--------|
| macOS | Apple Silicon | `aarch64-apple-darwin` |
| macOS | Intel | `x86_64-apple-darwin` |
| Ubuntu / Debian | x86_64 | `x86_64-unknown-linux-gnu` |
| Alpine (containers) | x86_64 | `x86_64-unknown-linux-musl` |
| Alpine (containers) | ARM64 | `aarch64-unknown-linux-musl` |
| Windows (WSL / native) | x86_64 | `x86_64-pc-windows-gnu` |

---

## Installation

Download the binary for your platform from the [latest release](../../releases/latest) and put it somewhere on your `PATH`.

### Linux / macOS (one-liner)

```sh
VERSION=$(curl -fsSL https://api.github.com/repos/CaDi-Team/diegops-tools/releases/latest | grep '"tag_name"' | cut -d'"' -f4)
TARGET=aarch64-apple-darwin   # see platform table above

curl -fsSL "https://github.com/CaDi-Team/diegops-tools/releases/download/${VERSION}/diegops-${VERSION}-${TARGET}.tar.gz" \
  | tar -xz
sudo mv diegops /usr/local/bin/diegops
```

### Alpine / containers (musl static)

```sh
VERSION=$(curl -fsSL https://api.github.com/repos/CaDi-Team/diegops-tools/releases/latest | grep '"tag_name"' | cut -d'"' -f4)
curl -fsSL "https://github.com/CaDi-Team/diegops-tools/releases/download/${VERSION}/diegops-${VERSION}-x86_64-unknown-linux-musl.tar.gz" \
  | tar -xz -C /usr/local/bin
```

### Windows (PowerShell)

```powershell
$repo    = "CaDi-Team/diegops-tools"
$version = (Invoke-RestMethod "https://api.github.com/repos/$repo/releases/latest").tag_name
$url     = "https://github.com/$repo/releases/download/$version/diegops-$version-x86_64-pc-windows-gnu.zip"
Invoke-WebRequest -Uri $url -OutFile diegops.zip
Expand-Archive -Path diegops.zip -DestinationPath "$env:USERPROFILE\bin" -Force
```

### Updating

```sh
diegops update
```

Fetches the latest release, downloads the binary for your platform, and replaces itself in place. Idempotent — prints "Already up to date" if current.

> **Permissions:** if the binary lives in `/usr/local/bin`, you may need `sudo diegops update`. The CLI will tell you.

---

## Commands at a Glance

```
diegops version              Print version
diegops update               Self-update to latest release
diegops cadi                 Show the DiegOps hero screen
diegops help                 Show help

diegops repo ...             Manage git repository workspace
diegops vault ...            Pull Vault secrets into .env files
diegops auth ...             Manage authentication tokens
diegops devtools ...         Developer workstation setup (git, gpg, ssh)
diegops sync ...             Cloud-sync config files to GitHub
diegops tool ...             Manage DevOps CLI tools
diegops ktool ...            Manage and run ktool (karluiz tools)

diegops <tool> <args>        Run a managed tool (gh, vault, terraform, etc.)
```

---

## `diegops repo` — Workspace Management

Clone and manage a set of git repositories defined in a YAML config file.

```
diegops repo init                          Create sample config at ~/.diegops/repos.yaml
diegops repo apply [--path P] [--config F] Clone all missing repos (idempotent)
diegops repo list [--config F]             Show repos already cloned locally
diegops repo list-diff [--config F]        Show repos in config but not yet cloned
```

**Config** (`~/.diegops/repos.yaml`):

```yaml
targets:
  - path: $HOME/github/my-org/products
    repos:
      - git@github.com:my-org/web-front.git
      - git@github.com:my-org/api-service.git
```

```sh
diegops repo init          # create sample config
diegops repo apply         # clone everything
diegops repo list-diff     # see what's missing
```

> Override config path with `--config <path>` or `$DIEGOPS_REPOS_CONFIG`.

---

## `diegops vault` — Secret Management

Pull secrets from HashiCorp Vault KV v2 and write `.env` files.

```
diegops vault init                          Create sample config at ~/.diegops/repo-vault.yaml
diegops vault apply [--path P] [--config F] Pull secrets and write .env files (idempotent)
diegops vault list [--config F]             Show targets with .env files
diegops vault list-diff [--config F]        Show targets missing .env files
```

**Config** (`~/.diegops/repo-vault.yaml`):

```yaml
targets:
  - path: $HOME/github/my-org/my-app
    secrets:
      - vault_path: secret/my-app/database
        keys: [username, password]
      - vault_path: secret/my-app/api
        keys: "*"
```

**Requires:** `vault` CLI on PATH, `VAULT_ADDR` set, authenticated session.

> `.env` files use `KEY="value"` format. `.gitignore` is auto-updated. Content-aware skip avoids unnecessary writes.

---

## `diegops auth` — Credential Management

Manage authentication tokens for external services.

```
diegops auth gh login <PAT>     Validate and store a GitHub token
diegops auth gh logout          Remove stored GitHub token
diegops auth gh whoami          Show authenticated GitHub user and scopes
diegops auth status             Show status for all providers (GitHub + ktool kenv)
diegops auth logout             Remove all stored tokens
```

**Storage:** `~/.diegops/tokens/gh.json` (0600 permissions on Unix).

**Resolution order:** stored file > `$GITHUB_TOKEN` env var > unauthenticated.

> `auth status` also shows ktool's kenv token status (read-only from `~/.ktool/tokens/`).

---

## `diegops sync` — Cloud Config Backup

Back up and restore `~/.diegops/` config files to a private GitHub repo.

```
diegops sync push       Upload local configs to GitHub
diegops sync pull       Download configs from GitHub
diegops sync status     Show diff between local and cloud
```

**How it works:**
- First `push` auto-creates a private repo `diegops-{your_github_username}-memory`
- Syncs all files in `~/.diegops/` **except** `tokens/` and `bin/` (security + size)
- Uses GitHub Contents API — no local git clone needed
- `push` overwrites cloud; `pull` overwrites local (explicit intent)
- Content-aware skip: unchanged files are not re-uploaded/downloaded

**Requires:** GitHub token (`diegops auth gh login` first).

---

## `diegops tool` — Managed DevOps Toolbox

Download, update, and run popular DevOps CLIs without installing them globally. All binaries live in `~/.diegops/bin/`.

```
diegops tool list               Show available tools and install status
diegops tool install <name>     Install latest version of a tool
diegops tool update             Update all installed tools
diegops tool update <name>      Update a specific tool
diegops tool remove <name>      Remove an installed tool
diegops <tool> <args>           Run a managed tool directly
```

### Available Tools

| Tool | Description | Config |
|------|-------------|--------|
| `gh` | GitHub CLI | `~/.config/gh/` |
| `vault` | HashiCorp Vault | stateless (env vars) |
| `terraform` | Infrastructure as code | `.terraform/` (per-project) |
| `helm` | Kubernetes package manager | `~/.config/helm/` |
| `k9s` | Kubernetes TUI | `~/.kube/` |
| `kubectl` | Kubernetes CLI | `~/.kube/` |
| `jq` | JSON processor | stateless |
| `yq` | YAML processor | stateless |
| `trivy` | Security scanner | `~/.cache/trivy/` |
| `trippy` | Network diagnostic | stateless |

### Examples

```sh
diegops tool list                    # see what's available
diegops tool install jq              # install jq
diegops jq '.name' package.json      # use it directly
diegops tool install gh              # install GitHub CLI
diegops gh pr list                   # use it
diegops tool update                  # update everything
```

> Tools are downloaded from GitHub Releases (or vendor CDN for kubectl). Each tool's release asset naming is handled automatically.

---

## `diegops ktool` — Karluiz Tools

Manage and proxy the ktool CLI (karluiz tools) as a sidecar binary.

```
diegops ktool update        Download/update ktool to ~/.diegops/bin/ktool
diegops ktool <args>        Forward args to the managed ktool binary
```

```sh
diegops ktool update              # install/update ktool
diegops ktool kenv list           # list kenv secrets
diegops ktool auth kenv login T   # authenticate with kenv
```

---

## `diegops devtools` — Developer Workstation Setup

Automate identity and tooling configuration for a fresh workstation.

**Identity resolution:** `--name`/`--email` flags > `git config` values > GitHub API (`gh`).

### git

```
diegops devtools git set [--name NAME] [--email EMAIL]
```

Sets `git config --global user.name` and `user.email`.

### gpg

```
diegops devtools gpg init [--name NAME] [--email EMAIL]   Generate GPG identity config
diegops devtools gpg set                                   Full setup: key + GitHub + signing
diegops devtools gpg restart                               Restart gpg-agent
```

### ssh

```
diegops devtools ssh list                                   List local + GitHub SSH keys
diegops devtools ssh config                                 Print ~/.ssh/config
diegops devtools ssh create [--name N] [--type T] [--email E]   Create SSH key
```

---

## `diegops cadi`

Displays the DiegOps hero screen with ASCII art branding. No arguments, no side effects.

---

## Directory Layout

```
~/.diegops/
├── bin/                 # Managed tool binaries (ktool, gh, jq, etc.)
├── tokens/              # Auth tokens (gh.json) — 0600 permissions
├── repos.yaml           # Repository workspace config
└── repo-vault.yaml      # Vault secrets config
```

---

## Development

### Prerequisites

- [Rust stable](https://rustup.rs)

### Build & Test

```sh
cargo build                     # debug build
cargo build --release           # release build
cargo test                      # run all tests
cargo fmt --check               # formatting gate
cargo clippy -- -D warnings     # lint gate
```

### Cutting a Release

```sh
git tag v1.2.3
git push origin v1.2.3
```

GitHub Actions builds all platform binaries and publishes a GitHub Release. All builds run on self-hosted runners (`cadi-hq-runner-dind-set-v2`). Cross-compilation uses `cargo-zigbuild`.
