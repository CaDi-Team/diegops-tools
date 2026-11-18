# Secrets backends: Infisical by default, Vault/OpenBao optional

Status: accepted · Date: 2026-11-18

## Context

`diegops` read and wrote secrets by shelling out to the `vault` CLI. The
address (`https://cadi-vault.cadi-labs.com`) was hard-coded in the managed shell
config, and every command needed `VAULT_ADDR` plus a live Vault session. The
personal secrets (`dpinto-config/…` for `secrets`/`vault` and `dpinto-work/…` for
gnupg, ssh and sops-age keys) are moving off the company OpenBao to a personal
Infisical Cloud org. CaDi company secrets stay on OpenBao.

## Decision

1. A `SecretsBackend` trait (`src/secrets_backend/mod.rs`) with `get`, `list` and
   `put` over **logical paths** `<mount>/<path>`, the same notation that
   `secrets.yaml` and `repo-vault.yaml` already use in `vault_path`. A `put` is an
   upsert: keys that are not in the call keep their values.
2. Two implementations, both over plain HTTPS with `ureq`/rustls, so no external CLI
   is needed:
   - **Infisical** (the default): `POST /api/v1/auth/universal-auth/login`,
     `GET /api/v4/secrets`, `PATCH /api/v4/secrets/batch` with `mode: upsert`, and
     `GET`/`POST /api/v2/folders`.
   - **Vault-compatible** (Vault or OpenBao KV v2): `X-Vault-Token`,
     `X-Vault-Namespace`, an optional extra CA certificate, and `GET`/`POST
     <mount>/data/<path>` and `LIST <mount>/metadata/<path>`. OIDC works through
     `bao login -method=oidc` or `vault login -method=oidc`, and `diegops` reads the
     resulting `~/.vault-token`.
3. Backend selection, from highest to lowest priority: the `--backend` flag, then
   `$DIEGOPS_SECRETS_BACKEND`, then `backend` in the config file, then `infisical`.
   No address is built in. Vault fails with "no Vault address configured" unless one
   is set.
4. Config file: `$DIEGOPS_CONFIG`, else `$XDG_CONFIG_HOME/diegops/config.toml`, else
   `~/.config/diegops/config.toml`. Unknown keys are rejected.
5. Path mapping to Infisical:
   - A logical `<mount>/<a>/<b>` maps to project `project_id` of the mount (or the
     default), environment `environment` (default `prod`), and folder
     `<folder or /<mount>>/<a>/<b>`. Each key becomes one Infisical secret.
   - Folder segments are sanitised to `[A-Za-z0-9_-]`, so `.` and other characters
     become `_`. `migrate` aborts before writing anything if two source paths
     collide after sanitising.
6. Value codec (`codec.rs`): Infisical trims values on the server. Any value that
   is multi-line, has leading or trailing whitespace, contains control characters,
   or already starts with the marker is stored as `diegops:b64:` followed by the
   base64 of the value. Plain one-line values are stored verbatim, so they stay
   readable in the UI. Binary files are already base64 in `secrets.yaml` data.
7. `diegops kv migrate <prefix>… [--dry-run]`:
   - It plans everything first: it lists the source recursively, maps each path,
     checks for collisions and invalid key names (Infisical forbids `:` and `/`),
     and compares SHA-256 hashes of the values in memory.
   - It then upserts only the changed keys and re-reads both sides to verify every
     key by SHA-256.
   - It is idempotent and never deletes anything. Output shows only paths and
     counts, never values.
8. The existing commands (`secrets push`/`pull`/`status`, `vault apply`, and the
   pre-flight check in `all`/`bootstrap`) now use the selected backend. The
   hard-coded `VAULT_ADDR` export was removed from the managed shell config.

## Infisical Cloud Free tier (checked 2026-11-18)

Source: <https://infisical.com/pricing>.

- 5 identities (human users and machine identities together), unlimited projects,
  3 environments, 10 secret syncs.
- **No secret versioning or point-in-time recovery, and no audit log retention, on
  Free.** Keep the OpenBao copy until verification passes, and keep an offline
  backup of the gnupg and keystore material.
- No RBAC, no SSO, no IP allowlisting.
- Default rate limits, from the Infisical source `backend/src/server/config/rateLimiter.ts`:
  60 reads/min, 200 writes/min, and 60 secret ops/min. The client retries on 429 and
  5xx, honouring `Retry-After`.
- Request bodies are capped at Fastify's default 1 MB per request. No separate cap
  on value size was found. Keystores and GPG keys are a few KB, so even after
  base64 they are far below the cap. Very large files (over about 700 KB raw) would
  need to be split.

## Consequences

- Infisical credentials: a machine identity with Universal Auth. The `client_id`
  goes in `config.toml`. The client secret comes from
  `$INFISICAL_UNIVERSAL_AUTH_CLIENT_SECRET`, `client_secret_command` (for example
  the OS keyring), or `client_secret_file`, which must be mode 0600.
  `INFISICAL_TOKEN` overrides all of them.
- `kv put` refuses inline values (`KEY=value`) to keep them out of shell history
  and the process list. Use `KEY=@file` or `KEY=-` instead.
- With the Infisical backend, a KV v2 path that is both a secret and a parent
  becomes one folder that holds both its own keys and its sub-folders.

## Example `~/.config/diegops/config.toml`

```toml
backend = "infisical"

[infisical]
# site_url = "https://eu.infisical.com"   # if the org is in the EU region
project_id = "<personal project id>"
environment = "prod"
client_id = "<machine identity client id>"
client_secret_command = ["secret-tool", "lookup", "service", "infisical", "account", "diegops"]
# client_secret_file = "~/.config/diegops/infisical-client-secret"   # chmod 600

# Optional per-mount overrides (default folder is /<mount>)
# [[infisical.mounts]]
# mount = "dpinto-work"
# folder = "/work"

[vault]   # only for --backend vault and for `kv migrate`
address = "https://<openbao address>"
namespace = "personal"
# ca_cert = "~/.config/diegops/openbao-ca.pem"
```
