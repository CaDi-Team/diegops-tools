# diegops

[![CI](https://github.com/CaDi-Team/diegops-tools/actions/workflows/ci.yml/badge.svg)](https://github.com/CaDi-Team/diegops-tools/actions/workflows/ci.yml)
[![License: Proprietary](https://img.shields.io/badge/License-Proprietary-red.svg)](LICENSE)

Personal productivity CLI for humans and containers.

## Supported platforms

| OS | Architecture | Target | Build tool |
|----|-------------|--------|------------|
| Windows (WSL / native) | x86_64 | `x86_64-pc-windows-gnu` | cross (MinGW) |
| macOS | Intel | `x86_64-apple-darwin` | cargo-zigbuild |
| macOS | Apple Silicon | `aarch64-apple-darwin` | cargo-zigbuild |
| Ubuntu / Debian | x86_64 | `x86_64-unknown-linux-gnu` | native |
| Alpine (containers) | x86_64 | `x86_64-unknown-linux-musl` | cross (musl) |
| Alpine (containers) | ARM64 | `aarch64-unknown-linux-musl` | cross (musl) |

> **Windows note:** the release binary targets `x86_64-pc-windows-gnu` (MinGW toolchain). It is fully
> standalone — no MinGW runtime DLLs are required. MSVC cross-compilation from Linux is not possible.

## Installation

Download the binary for your platform from the [latest release](../../releases/latest) and put it somewhere on your `PATH`.

### Linux / macOS — one-liner (auto-detects latest version and platform)

```sh
curl -fsSL "https://api.github.com/repos/CaDi-Team/diegops-tools/releases/latest" \
  | grep '"browser_download_url"' \
  | grep "$(uname -s | tr '[:upper:]' '[:lower:]' | sed 's/darwin/apple-darwin/;s/linux/unknown-linux-gnu/')-$(uname -m | sed 's/x86_64/x86_64/;s/arm64/aarch64/')" \
  | cut -d'"' -f4 \
  | xargs curl -fsSL \
  | tar -xz
sudo mv diegops /usr/local/bin/diegops
```

Or pick a specific version and target manually:

```sh
VERSION=$(curl -fsSL https://api.github.com/repos/CaDi-Team/diegops-tools/releases/latest | grep '"tag_name"' | cut -d'"' -f4)
TARGET=x86_64-unknown-linux-gnu   # see table above for your target

curl -fsSL "https://github.com/CaDi-Team/diegops-tools/releases/download/${VERSION}/diegops-${VERSION}-${TARGET}.tar.gz" \
  | tar -xz
sudo mv diegops /usr/local/bin/diegops
```

**Alpine / musl (containers):**
```sh
VERSION=$(curl -fsSL https://api.github.com/repos/CaDi-Team/diegops-tools/releases/latest | grep '"tag_name"' | cut -d'"' -f4)
ARCH=$(uname -m | sed 's/x86_64/x86_64/;s/aarch64/aarch64/')
curl -fsSL "https://github.com/CaDi-Team/diegops-tools/releases/download/${VERSION}/diegops-${VERSION}-${ARCH}-unknown-linux-musl.tar.gz" \
  | tar -xz -C /usr/local/bin
```

### Windows (PowerShell)

```powershell
$repo    = "CaDi-Team/diegops-tools"
$version = (Invoke-RestMethod "https://api.github.com/repos/$repo/releases/latest").tag_name
$target  = "x86_64-pc-windows-gnu"
$url     = "https://github.com/$repo/releases/download/$version/diegops-$version-$target.zip"

Invoke-WebRequest -Uri $url -OutFile diegops.zip
Expand-Archive -Path diegops.zip -DestinationPath "$env:USERPROFILE\bin" -Force
# Ensure $env:USERPROFILE\bin is on your PATH
```

### Container / CI (one-liner, always latest)

```sh
# Alpine (musl static — no apk packages needed beyond curl and tar)
apk add --no-cache curl tar
VERSION=$(curl -fsSL https://api.github.com/repos/CaDi-Team/diegops-tools/releases/latest | grep '"tag_name"' | cut -d'"' -f4)
curl -fsSL "https://github.com/CaDi-Team/diegops-tools/releases/download/${VERSION}/diegops-${VERSION}-x86_64-unknown-linux-musl.tar.gz" \
  | tar -xz -C /usr/local/bin
```

```sh
# Ubuntu / Debian
VERSION=$(curl -fsSL https://api.github.com/repos/CaDi-Team/diegops-tools/releases/latest | grep '"tag_name"' | cut -d'"' -f4)
curl -fsSL "https://github.com/CaDi-Team/diegops-tools/releases/download/${VERSION}/diegops-${VERSION}-x86_64-unknown-linux-gnu.tar.gz" \
  | tar -xz -C /usr/local/bin
```

## Updating

Once installed, update to the latest release with:

```sh
diegops update
```

This fetches the latest tag from GitHub Releases, downloads the binary for your current platform, and replaces the running binary in place. The command is idempotent — running it when already on the latest version prints `Already up to date` and exits 0.

> **Container note:** the binary writes the new file alongside itself, so the directory must be writable. If the binary lives in a read-only layer, copy it to a writable path first.

## Commands

```
diegops version                                    Print version information
diegops update                                     Update to the latest released version
diegops help                                       Show help and available commands
diegops --help                                     Same as help
diegops --version                                  Same as version

diegops repo init                                  Create sample config at ~/.diegops/repos.yaml
diegops repo apply                                 Clone all missing repos (idempotent)
diegops repo apply --path <prefix>                 Clone only repos under this path prefix
diegops repo apply --config <file>                 Use a custom repos.yaml
diegops repo list                                  Show repos already cloned locally
diegops repo list-diff                             Show repos in config but not yet cloned

diegops vault init                                 Create sample config at ~/.diegops/repo-vault.yaml
diegops vault apply                                Pull secrets and write .env files
diegops vault apply --path <prefix>                Pull only for targets under this path prefix
diegops vault apply --config <file>                Use a custom repo-vault.yaml
diegops vault list                                 Show targets that have a .env file
diegops vault list-diff                            Show targets missing a .env file
```

### `diegops repo` — repository workspace

Manages a set of git repositories defined in a YAML config file.

```
diegops repo init                        Create a sample config in ~/.diegops/
diegops repo apply                       Clone all missing repos (idempotent)
diegops repo apply --path <prefix>       Clone only repos under this path prefix
diegops repo apply --config <file>       Use a custom repos.yaml file
diegops repo list                        Show repos that are already cloned locally
diegops repo list-diff                   Show repos in config but NOT cloned locally
```

**Getting started:**

```sh
diegops repo init          # creates ~/.diegops/repos.yaml with a commented sample
# edit ~/.diegops/repos.yaml to add your repos
diegops repo apply         # clone everything
```

**Config file** (`~/.diegops/repos.yaml`) format:

```yaml
targets:
  - path: $HOME/github/cadilabs/products/cadibrain
    repos:
      - git@github.com:CaDi-Team/cadibrains-web-front.git
      - git@github.com:CaDi-Team/cadibrains-ai-service.git
```

**Default config location:** `~/.diegops/repos.yaml`
Override with `--config <path>` or `$DIEGOPS_REPOS_CONFIG`.

**Examples:**

```sh
# Clone everything in the config
diegops repo apply

# Clone only the cadibrain workspace
diegops repo apply --path '$HOME/github/cadilabs/products/cadibrain'

# See what's missing before cloning
diegops repo list-diff

# See what's already cloned
diegops repo list
```

> **Idempotency:** `apply` skips repos whose directory already contains a `.git` folder — safe to run repeatedly.

### `diegops vault` — secret management

Pulls secrets from [HashiCorp Vault](https://www.vaultproject.io/) KV v2 and writes `.env` files into your repository directories.

```
diegops vault init                        Create a sample config in ~/.diegops/
diegops vault apply                       Pull secrets and write .env files (idempotent)
diegops vault apply --path <prefix>       Pull only for targets under this path prefix
diegops vault apply --config <file>       Use a custom repo-vault.yaml file
diegops vault list                        Show targets that already have a .env file
diegops vault list-diff                   Show targets in config but missing a .env file
```

**Getting started:**

```sh
diegops vault init         # creates ~/.diegops/repo-vault.yaml with a commented sample
# edit ~/.diegops/repo-vault.yaml to add your Vault paths and keys
diegops vault apply        # pull secrets and write .env files
```

**Config file** (`~/.diegops/repo-vault.yaml`) format:

```yaml
targets:
  - path: $HOME/github/my-org/products/my-product
    secrets:
      - vault_path: secret/my-product/database
        keys:
          - username
          - password
      - vault_path: secret/my-product/api
        keys: "*"
```

**Default config location:** `~/.diegops/repo-vault.yaml`
Override with `--config <path>` or `$DIEGOPS_VAULT_CONFIG`.

**Examples:**

```sh
# Pull all secrets defined in the config
diegops vault apply

# Pull only secrets for one workspace
diegops vault apply --path '$HOME/github/my-org/products/my-product'

# See which targets are missing a .env file
diegops vault list-diff

# See which targets already have a .env file
diegops vault list
```

**Prerequisites:**
- The `vault` CLI must be installed and on your `PATH`.
- You must be authenticated (`vault login`).
- `VAULT_ADDR` must be set in your environment.

**Key behavior:**
- `vault_path` uses the **logical** Vault path (no `/data/` segment — Vault KV v2 adds it automatically).
- `keys` can be a list of specific key names or `"*"` to pull all keys from the path.
- Values are written as-is with case-preserved key names.
- `.env` is fully overwritten on each run; content-aware skip avoids unnecessary writes.
- `.gitignore` is automatically updated to include `.env` if not already present.

> **Idempotency:** `apply` compares the new `.env` content with the existing file and skips the write if unchanged — safe to run repeatedly.

## Development

### Prerequisites

- [Rust stable](https://rustup.rs) (`rustup install stable`)

### Build

```sh
cargo build
cargo build --release
```

### Test

```sh
cargo test
```

### Lint

```sh
cargo fmt --check
cargo clippy -- -D warnings
```

### Cutting a release

Push a tag that matches `vX.Y.Z`:

```sh
git tag v1.0.0
git push origin v1.0.0
```

GitHub Actions will build all platform binaries and publish them as a GitHub Release.

## Cross-compilation (local)

Install [`cross`](https://github.com/cross-rs/cross) and Docker:

```sh
cargo install cross
cross build --release --target x86_64-unknown-linux-musl
cross build --release --target aarch64-unknown-linux-musl
```
