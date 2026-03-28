# Tool Manager — Design Spec

**Date:** 2026-03-28
**Status:** Approved

## Overview

Add `diegops tool` commands to manage a curated set of DevOps CLI tools — download, update, remove, and proxy them through diegops. All binaries live in `~/.diegops/bin/`. Users invoke tools via `diegops <tool> <args>`.

## Commands

| Command | Description |
|---------|-------------|
| `diegops tool list` | Show all available tools with install status and version |
| `diegops tool install <name>` | Install latest version of a tool |
| `diegops tool update` | Update all installed tools |
| `diegops tool update <name>` | Update a specific tool |
| `diegops tool remove <name>` | Remove an installed tool |
| `diegops <tool> <args>` | Passthrough to managed binary (e.g., `diegops gh pr list`) |

## Tool Registry

A static `&[ToolDef]` array compiled into the binary. No network config to fetch — adding a tool is a code change.

### GitHub Releases Tools

| Name | Repo | Description | Binary name | Asset pattern | Config |
|------|------|-------------|-------------|---------------|--------|
| gh | `cli/cli` | GitHub CLI | `gh` | `gh_{v}_{os}_{arch}.tar.gz` (binary inside `gh_{v}_{os}_{arch}/bin/gh`) | `~/.config/gh/` |
| vault | `hashicorp/vault` | Secret management | `vault` | `vault_{v}_{os}_{arch}.zip` | stateless (env vars) |
| terraform | `hashicorp/terraform` | Infrastructure as code | `terraform` | `terraform_{v}_{os}_{arch}.zip` | `.terraform/` (per-project) |
| helm | `helm/helm` | Kubernetes package manager | `helm` | `helm-v{v}-{os}-{arch}.tar.gz` (binary inside `{os}-{arch}/helm`) | `~/.config/helm/` |
| k9s | `derailed/k9s` | Kubernetes TUI | `k9s` | `k9s_{os}_{arch}.tar.gz` | `~/.kube/` |
| jq | `jqlang/jq` | JSON processor | `jq` | `jq-{os}-{arch}` (bare binary) | stateless |
| yq | `mikefarah/yq` | YAML processor | `yq` | `yq_{os}_{arch}.tar.gz` | stateless |
| trivy | `aquasecurity/trivy` | Security scanner | `trivy` | `trivy_{v}_{os}-{arch}.tar.gz` | `~/.cache/trivy/` |
| trippy | `fujiapple852/trippy` | Network traceroute | `trip` | `trippy-{v}-{target}.tar.gz` | stateless |

### Special Case: kubectl

Not on GitHub Releases. Uses Google's CDN:
- Latest version: `GET https://dl.k8s.io/release/stable.txt`
- Download: `https://dl.k8s.io/release/{version}/bin/{os}/{arch}/kubectl`
- Bare binary (no archive)
- Config: `~/.kube/`

## ToolDef Structure

```rust
struct ToolDef {
    /// CLI name used in commands (e.g., "gh", "vault")
    name: &'static str,
    /// Human-readable description
    description: &'static str,
    /// Source type
    source: ToolSource,
    /// Name of the binary file inside the archive (may differ from tool name, e.g., "trip" for trippy)
    binary_name: &'static str,
    /// Archive format
    archive: ArchiveFormat,
    /// Path to binary inside archive (if nested), e.g., "linux-amd64/helm"
    /// Empty string means binary is at archive root.
    binary_path_template: &'static str,
    /// Config location hint for `tool list` display
    config_hint: &'static str,
}

enum ToolSource {
    /// GitHub Releases — uses API to find latest, asset name pattern for download
    GitHub {
        repo: &'static str,          // "cli/cli"
        asset_template: &'static str, // "gh_{version}_{os}_{arch}.tar.gz"
    },
    /// Custom URL pattern
    Url {
        latest_url: &'static str,     // URL that returns latest version string
        download_template: &'static str, // URL with {version}, {os}, {arch} placeholders
    },
}

enum ArchiveFormat {
    TarGz,
    Zip,
    Bare, // raw binary, no archive
}
```

## Platform Mapping

Each tool's asset pattern uses `{os}` and `{arch}` placeholders. These map from Rust's compile-time target to vendor naming conventions.

Most tools use:
- `{os}`: `linux`, `darwin`, `windows`
- `{arch}`: `amd64`, `arm64`

Some tools (trippy) use full Rust target triples. The mapping is per-tool via the asset template.

```rust
fn platform_os() -> &'static str {
    if cfg!(target_os = "macos") { "darwin" }
    else if cfg!(target_os = "linux") { "linux" }
    else if cfg!(target_os = "windows") { "windows" }
    else { "unknown" }
}

fn platform_arch() -> &'static str {
    if cfg!(target_arch = "x86_64") { "amd64" }
    else if cfg!(target_arch = "aarch64") { "arm64" }
    else { "unknown" }
}
```

## Install Flow

1. Look up tool in registry by name — error if unknown
2. Determine latest version (GitHub API or custom URL)
3. Check if already installed at that version (run `~/.diegops/bin/{binary} --version`)
4. Build download URL from template + version + platform
5. Download binary/archive
6. Extract if needed (tar.gz, zip) — find binary by `binary_path_template` or `binary_name`
7. Write to `~/.diegops/bin/{binary_name}`, chmod 755
8. Verify: run `{binary} --version` to confirm it works

## Update Flow

### `diegops tool update` (all)
1. Scan `~/.diegops/bin/` for installed tools (match filenames against registry)
2. For each installed tool, check latest version
3. Skip if current, download + replace if newer
4. Progress to stderr, summary to stdout

### `diegops tool update <name>`
Same as install — install is idempotent (installs if missing, updates if outdated).

## Remove Flow

1. Look up binary name from registry
2. Delete `~/.diegops/bin/{binary_name}`
3. Print confirmation or "not installed"

## List Output

```
Available tools:

  Name        Version     Description               Config
  ────        ───────     ───────────               ──────
  gh          v2.50.0     GitHub CLI                ~/.config/gh/
  vault       (not installed) Secret management     stateless
  terraform   v1.9.0      Infrastructure as code    .terraform/ (per-project)
  helm        v3.15.0     Kubernetes package mgr    ~/.config/helm/
  k9s         (not installed) Kubernetes TUI        ~/.kube/
  kubectl     v1.30.0     Kubernetes CLI            ~/.kube/
  jq          v1.7.1      JSON processor            stateless
  yq          (not installed) YAML processor        stateless
  trivy       (not installed) Security scanner      ~/.cache/trivy/
  trippy      (not installed) Network traceroute    stateless

Installed: 4/10  |  Run 'diegops tool install <name>' to add tools
```

Version detection: run `~/.diegops/bin/{binary} --version` (or `version` for some tools), parse output.

## Passthrough

For each tool in the registry, diegops registers a top-level command that forwards to the managed binary. This reuses the exact same passthrough pattern as ktool:

```rust
// In main.rs, after all explicit commands:
// Check if first arg matches a registered tool name
// If yes, forward remaining args to ~/.diegops/bin/{tool}
```

Implementation: instead of adding a Clap variant per tool, handle unknown commands in the `None`/default match arm by checking the tool registry.

**If tool not installed:** error with `"gh is not installed. Run 'diegops tool install gh' first."`

## Version Detection

Each tool has its own version output format. Rather than parsing each one precisely, use a simple heuristic:
1. Run `{binary} --version` (or `{binary} version` as fallback)
2. Extract the first token that looks like a semver: regex `v?\d+\.\d+\.\d+`
3. Compare with latest release tag (also normalized to strip `v` prefix)

## Error Handling

| Condition | Behavior |
|-----------|----------|
| Unknown tool name | Exit 1: "Unknown tool '{name}'. Run 'diegops tool list' to see available tools." |
| No GitHub token (for private repos) | Fall back to unauthenticated (all tools are public) |
| Network error | Exit 2: "Could not reach GitHub API. Check your connection." |
| No matching asset for platform | Exit 1: "{tool} does not have a release for {os}/{arch}." |
| Tool not installed on passthrough | Exit 1: "{tool} is not installed. Run 'diegops tool install {tool}' first." |
| Version detection fails | Show "(installed, version unknown)" in list |

## Project Structure

```
src/commands/
├── tool.rs               # tool list/install/update/remove commands + ToolDef registry
└── ...                   # existing modules unchanged
```

The passthrough logic lives in `main.rs` — checks unrecognized commands against the tool registry.

## Testing

**Unit tests (in tool.rs):**
- Platform mapping returns valid values
- Asset URL template expansion
- Version extraction from various formats
- Tool lookup by name

**Integration tests:**
- `diegops tool list` exits 0, shows known tools
- `diegops tool --help` exits 0
- `diegops tool install --help` exits 0

No network tests — install/update require live downloads.

## Idempotency

- `install` when already at latest: prints "already up to date", exits 0
- `update` when all current: prints "all tools up to date", exits 0
- `remove` when not installed: prints "not installed", exits 0
- All commands safe to run repeatedly

## Future Considerations (not in v1)

- `diegops tool install gh@2.50.0` — pinned versions
- Tool groups / presets (`diegops tool install --preset devops`)
- `diegops tool sync` — integrate with cloud-sync to remember installed tools
- User-defined tools via config file
