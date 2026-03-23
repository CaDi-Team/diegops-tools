# diegops

[![CI](https://github.com/dpinto-config/diegops-tools/actions/workflows/ci.yml/badge.svg)](https://github.com/dpinto-config/diegops-tools/actions/workflows/ci.yml)
[![License: Proprietary](https://img.shields.io/badge/License-Proprietary-red.svg)](LICENSE)

Personal productivity CLI for humans and containers.

## Supported platforms

| OS | Architecture | Target |
|----|-------------|--------|
| Windows (WSL / native) | x86_64 | `x86_64-pc-windows-msvc` |
| macOS | Intel | `x86_64-apple-darwin` |
| macOS | Apple Silicon | `aarch64-apple-darwin` |
| Ubuntu / Debian | x86_64 | `x86_64-unknown-linux-gnu` |
| Alpine (containers) | x86_64 | `x86_64-unknown-linux-musl` |
| Alpine (containers) | ARM64 | `aarch64-unknown-linux-musl` |

## Installation

Download the binary for your platform from the [latest release](../../releases/latest) and put it somewhere on your `PATH`.

### Linux / macOS

Replace `<version>` and `<target>` with the values that match your system (see the table above):

```sh
VERSION=v0.1.0
TARGET=x86_64-unknown-linux-gnu   # adjust to your target

curl -fsSL "https://github.com/dpinto-config/diegops-tools/releases/download/${VERSION}/diegops-${VERSION}-${TARGET}.tar.gz" \
  | tar -xz

# Move the binary to a directory on your PATH, e.g.:
mv diegops /usr/local/bin/diegops
chmod +x /usr/local/bin/diegops
```

**Alpine / container (musl):**
```sh
TARGET=x86_64-unknown-linux-musl   # or aarch64-unknown-linux-musl
```

**macOS Apple Silicon:**
```sh
TARGET=aarch64-apple-darwin
```

### Windows (PowerShell)

```powershell
$version = "v0.1.0"
$target  = "x86_64-pc-windows-msvc"
$url     = "https://github.com/dpinto-config/diegops-tools/releases/download/$version/diegops-$version-$target.zip"

Invoke-WebRequest -Uri $url -OutFile diegops.zip
Expand-Archive -Path diegops.zip -DestinationPath "$env:USERPROFILE\bin"
# Ensure $env:USERPROFILE\bin is on your PATH
```

### Container / CI (one-liner)

```sh
# Alpine
apk add --no-cache curl tar
VERSION=v0.1.0
curl -fsSL "https://github.com/dpinto-config/diegops-tools/releases/download/${VERSION}/diegops-${VERSION}-x86_64-unknown-linux-musl.tar.gz" \
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
