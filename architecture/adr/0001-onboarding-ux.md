# 0001 — Onboarding and command UX model

- **Status**: Accepted (2026-11-21)
- **Date**: 2026-11-21
- **Deciders**: Diego Pinto (owner)
- **Supersedes**: —
- **Superseded by**: —
- **Related**: epic #55; issues #47 (this ADR), #48–#54; UX audit 2026-11-21 (phase 1 shipped in v1.7.0: #37–#46, #61)

## Context

diegops is a single Rust binary (clap 4) that sets up a developer machine: repos, `.env` files, secret files, shell, tools and AI-agent backups, with Infisical as default secrets backend and Vault/OpenBao optional. The usability audit (2026-11-21) found that a senior can use it but a junior cannot finish the first run without help. Phase 1 (v1.7.0) fixed help, errors, `doctor`, `explain`, completions and `shell init --dry-run/--yes`. The remaining work (#48–#54) changes the command surface, global flags, credential storage and onboarding flow. Those are structural and must be decided once, here, before code.

Current state (verified on `develop` after v1.7.0):

- **Naming overlaps.** Four secret-related commands: `vault` (project `.env` files, works with Infisical despite the name, `src/main.rs:146`), `kv` (raw backend read/write, `src/main.rs:155`), `secrets` (secret files such as `~/.ssh`, `src/main.rs:181`) and `agents hermes secrets`. `diegops tool` also manages a `vault` binary (`src/commands/tool.rs:170`), so "vault" means two things.
- **Flags are per command.** `--backend` exists only on `kv` (`src/main.rs:155`); `--dry-run` only on `kv migrate` and `shell init`; `--json` only on `doctor`. Commands that overwrite files (`vault apply`, `secrets pull`, `agents restore`) have no preview.
- **Config precedence is already flag > env > default file** for config paths (`src/commands/common.rs:61`) and for the backend (flag > `DIEGOPS_SECRETS_BACKEND` > `config.toml` > infisical, `src/main.rs:152`). The GitHub token is the exception: stored file > `GITHUB_TOKEN` (`src/commands/auth.rs:169`).
- **Non-TTY behaviour is inconsistent.** `shell init` treats a non-TTY stdin as "yes" (`src/commands/shell.rs:233`), so a piped/CI run executes sudo and `chsh` without consent; `auth gh login` reads stdin when not a TTY (`src/commands/auth.rs:275`).
- **Credentials at rest** are 0600 files: GitHub token in `~/.diegops/tokens/gh.json` (`src/commands/auth.rs:116-122`), Infisical client secret via `client_secret_file` or `client_secret_command` (`src/secrets_backend/config.rs:291-306`). No OS keyring.
- **Config is split** between `~/.config/diegops/config.toml` (backend) and `~/.diegops/*.yaml` (repos, secrets, repo-vault, agents). There is no `init`; each area has its own `… init` sample writer.

Research behind the options (prompt crates, onboarding patterns in gh/aws/gcloud/doctl/flyctl, keyring on musl, clig.dev) is in the audit, `C-research.md`.

## Decision Drivers

- A junior gets a working machine from `diegops init` with no web-UI trips except the ones a provider makes unavoidable.
- A senior or CI keeps a one-liner: every prompt has a flag and an env var, and nothing hangs without a TTY.
- Backward compatibility: existing scripts, config files and command names keep working for at least one minor release (maintenance rule: additive changes, no silent breakage).
- Secrets never in argv, logs, `--json` output or git.
- Single static binary: musl and Windows-gnu cross builds keep working; size growth is measured.

## Considered Options

### Option A: Document only (keep surface, add docs and `explain`)
- **Pros**: no code risk, no renames.
- **Cons**: the audit's top blockers stay (no wizard, "vault" means two things, no preview on overwriting commands, non-TTY auto-yes).

### Option B: One coherent UX model, additive, with compatibility aliases (this ADR)
- **Pros**: fixes the blockers; existing names, files and scripts keep working; each piece ships as its own small PR behind the rules below.
- **Cons**: more surface to keep in sync (flag + env + prompt + doctor + explain per setting); deprecation period to manage.

### Option C: Redesign as v2 (new command tree, new config, break old names)
- **Pros**: cleanest result.
- **Cons**: breaks every existing machine and script; violates the additive-change rule; large single change.

## Decision

We chose **Option B**: diegops adopts one UX model (rules D1–D9 below), introduced additively; old command names and config files keep working as hidden, warning aliases for at least one minor release.

### D1 — Command names (#48)
- `env` = project `.env` files (today `vault`). `files` = secret files like `~/.ssh` (today `secrets`). `kv` unchanged (raw backend access). `agents hermes secrets` unchanged (scoped under agents).
- `vault` and `secrets` stay as **hidden aliases** that print `warning: 'diegops vault' is now 'diegops env'` to stderr and then run. Removed no earlier than the second minor release after the rename (e.g. renamed in 1.8, removed in 1.10 at the earliest), and only with an explicit release note.
- `diegops tool … vault` (the binary) is unaffected; the ambiguity disappears once `vault` stops being a command name.

### D2 — Precedence
For every setting: **flag > env var > config file > interactive prompt > default**. The GitHub token follows it too: `--with-token`/stdin > `GITHUB_TOKEN`/`GH_TOKEN` > credential store > `gh auth token`. This reverses today's file > env order; the change is noted in the release notes.

### D3 — Interactivity
- Prompt only when stdin **and** stderr are TTYs and neither `CI` nor `DIEGOPS_NO_INPUT` is set, and `--no-input` was not passed.
- Otherwise a missing required value exits **2** naming the flag/env (`error: missing Infisical client ID\n  hint: pass --infisical-client-id or set INFISICAL_CLIENT_ID`).
- A confirmation without a TTY is **refused**, not assumed: exit 2 with `hint: pass --yes`. This fixes `shell init` auto-yes (`shell.rs:233`), which is a behaviour change for piped runs and is listed in the release notes.
- Secrets are never accepted as flag values; they come from stdin (`--…-stdin`), env, a file or a hidden prompt.

### D4 — Global flags (#49, #50)
Defined once on the root command, `global = true`, with the same meaning everywhere:
- `-y/--yes` accept defaults and confirmations; `--no-input` never prompt (implies failing on missing values, not `--yes`).
- `-n/--dry-run` on every command that writes files, runs sudo or writes to a backend; prints the plan and exits 0 without side effects. A command that cannot honour it rejects it explicitly.
- `--json` on read/status/list commands only (`doctor`, `env status`, `files status`, `kv list`, `kv get` names only, `repo list-diff`, `agents status`, `tool list`). Schema is documented, versioned with a top-level `"schema": 1`, and **never contains secret values**.
- `-q/--quiet`, `-v/--verbose`; `NO_COLOR` and `TERM=dumb` respected; progress and messages on stderr, data on stdout.
- `--backend` becomes global (today only on `kv`).

### D5 — `diegops init` wizard (#51)
- One entry point that orchestrates existing commands; it does not duplicate their logic. Steps: preflight → backend → backend credentials → GitHub → sync repo → repos → secret files → tools & shell → agents → review → apply → `doctor`.
- Each step: one-line "why" + `explain` pointer, prompt with default, **validate immediately** (backend login, `GET /user`), up to 3 retries.
- Idempotent and resumable: steps already satisfied (as `doctor` sees them) show ✓ and are skipped; `--reset <step>` redoes one.
- Review screen before writing; `--dry-run` stops there.
- Every wizard field has a flag and env var; a test asserts that, so the wizard and the non-interactive path cannot drift.
- Prompt crate: **`inquire`** (one crate only; NotTTY error, validators, masked input, pure Rust). Binary growth measured in the PR; budget +600 KB.

### D6 — Credential store (#51)
- `SecretStore` chain: env var → OS keyring (`keyring` crate; zbus secret-service on Linux, native on macOS/Windows) → 0600 file (today's behaviour) with a `warning:` once.
- `--credential-store=keyring|file|env` forces one. `doctor` reports which store is in use.
- Existing 0600 files keep working; migration to the keyring is offered, never forced.
- Gate: if the keyring crate breaks musl or Windows-gnu cross builds, ship the file store only and record it as an amendment to this ADR.

### D7 — Config layout (#52)
- Keep both locations (no move): `~/.config/diegops/config.toml` = how diegops connects (backend, credentials refs); `~/.diegops/*.yaml` = what diegops manages (repos, env, files, agents), synced by `diegops sync`.
- Add `diegops config path` (prints every resolved path and which rule chose it) and document the split in README and `explain config`.
- Convergence, if ever, needs its own ADR.

### D8 — `doctor` and `explain` contract
- `doctor` is the definition of "set up": `init` ends with it and skips what it reports ✓. Exit 1 on any ✗; `--json` schema per D4. A new setting ships with its doctor check.
- `explain` is the single glossary; errors and wizard steps point to it. A new user-facing term ships with its entry (already enforced by a test).

### D9 — Zero-web-UI onboarding is phase 3 and spike-gated (#53, #54)
- Infisical auto-setup (create project + machine identity + client secret) and GitHub device flow are **spikes first**. They must not depend on undocumented storage of another CLI, must state the permissions and free-tier limits they need, and must roll back on failure.
- The paste paths (Infisical Client ID/Secret, GitHub fine-grained PAT, `gh auth token`) stay supported permanently.

## Owner answers (2026-11-21)

All four open points accepted as recommended:
1. Non-TTY confirmations are refused (exit 2, `hint: pass --yes`) instead of assumed, including `shell init` (D3).
2. GitHub token follows D2: explicit input > `GITHUB_TOKEN`/`GH_TOKEN` > credential store > `gh auth token` (an env token now overrides the stored one).
3. OS keyring is in phase 2, gated on musl and Windows-gnu cross builds (D6).
4. Old names `vault`/`secrets` are removed no earlier than two minor releases after the rename, announced in release notes (D1).

## Consequences

### Positive
- A junior has one command (`init`) and one check (`doctor`); a senior/CI has flags and env for everything.
- Same flags mean the same thing everywhere; every overwriting command can be previewed.
- Piped/CI runs stop executing sudo/`chsh` without consent.
- Names match what commands do; "vault" stops meaning two things.

### Negative / Trade-offs
- Behaviour changes users may notice: GitHub token precedence (env now beats the stored file) and non-TTY confirmations now fail instead of proceeding. Both are listed in release notes; `--yes` restores the old non-interactive behaviour.
- More to maintain per setting: flag + env + prompt + doctor check + explain entry. Mitigated by sync tests.
- New dependencies (`inquire`, `keyring` + zbus) add binary size and cross-build risk; gated in D5/D6.
- Two names per renamed command during the deprecation window.

### Follow-ups Required
- [ ] #48 rename `vault`→`env`, `secrets`→`files` with hidden warning aliases (D1)
- [ ] #49 global `--yes/--no-input/--dry-run/--quiet/--verbose/--backend`, non-TTY refusal, token precedence (D2–D4)
- [ ] #50 `--json` on read/list commands with `schema: 1` (D4)
- [ ] #51 `init` wizard + `SecretStore` chain (D5, D6)
- [ ] #52 `config path` + docs (D7)
- [ ] #53, #54 spikes (D9)
- [ ] README: command map with new names and the precedence rule

## Validation

- A fresh-user run (temporary HOME, no prior config) reaches `doctor` with 0 ✗ using only `diegops init` and pasted credentials, with no other command typed.
- `diegops init --no-input …` with stdin closed completes or exits 2 naming the missing flag; never hangs (CI test).
- Every old command name still works in integration tests during the deprecation window.
- Release binaries for all 6 targets still build; size growth recorded in the PR.
- Review date: after #51 ships, or 2027-02-28, whichever comes first.
