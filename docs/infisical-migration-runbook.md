# Runbook: personal secrets from OpenBao to Infisical Cloud

Examples use the mounts `dpinto-config` and `dpinto-work`; replace them with your own.
If `VAULT_ADDR`/`VAULT_NAMESPACE` are exported in your shell they override `[vault]`
in `config.toml`: run `unset VAULT_ADDR VAULT_NAMESPACE` first.

Never paste a client secret, token or secret value into chat, an issue or a
commit. Every `diegops` command below prints paths and counts, never values.

## 1. Set up Infisical (Diego, in the web UI)

1. Sign up at <https://app.infisical.com> (US region) or
   <https://eu.infisical.com> (EU region). Enable 2FA.
2. Create the organisation (any name). Then create a Secrets Management
   project called **`personal`**.
3. Environments: keep only **`prod`** in use. The Free tier allows 3, and the
   default `dev`/`staging` can be left empty.
4. Go to Organization → Access Control → **Identities** → *Create identity*.
   Name it `diegops-workstation` with organisation role *Member*. Add
   **Universal Auth** and set these options:
   - Access-token TTL: 2592000 (30 days). The max TTL can stay at its default.
   - Optionally, restrict *Client Secret Trusted IPs* to your home or office.
5. Create a **Client Secret** for that identity, with a description and no
   expiry, or an expiry of 1 year. It is shown only once.
6. In project `personal`, go to Access Control → Machine Identities → *Add
   identity* and choose `diegops-workstation` with role **Admin**. The Free tier has
   no custom roles, and `migrate` must create folders.
7. Copy the **project ID** from Project Settings and the identity's **Client ID**.
   Neither is a secret.
8. Store the client secret locally, never in a repository and never in chat.
   - Preferred, using the OS keyring:
     `secret-tool store --label "Infisical diegops" service infisical account diegops`
     and paste the secret at the prompt.
   - Fallback, a private file (the secret never reaches shell history):
     ```sh
     install -m 600 /dev/null ~/.config/diegops/infisical-client-secret
     read -rs -p "Client secret: " S && printf '%s' "$S" > ~/.config/diegops/infisical-client-secret; unset S; echo
     ```
9. Write `~/.config/diegops/config.toml` as shown in
   [the README](../README.md#step-4-write-the-config-file) and
   [secrets-backends.md](secrets-backends.md), with `project_id`, `client_id`, the
   `client_secret_command` or `client_secret_file` line, and the `[vault]` table
   pointing at OpenBao with `namespace = "personal"`.

The identity counts as 1 of the 5 Free-tier identities, and your user counts as
another.

## 2. Check access (read-only)

```sh
diegops kv backend                      # -> backend: infisical
bao login -method=oidc                  # or vault login; writes ~/.vault-token
diegops kv --backend vault list dpinto-config
diegops kv --backend vault list dpinto-work
bao secrets list -ns=personal           # find any other personal mounts (signing keystores)
diegops kv list dpinto-config           # Infisical: empty or "no Infisical project" is a config error
```

## 3. Dry-run

```sh
diegops kv migrate dpinto-config dpinto-work <other-mounts…> --dry-run
```

Check the plan before going further:

- Every line should read `PLAN … -> <project>/prod:/<mount>/…` and the total should
  match what you expect.
- A *collision* or an *invalid key* error aborts the run before anything is
  written. Fix it by renaming the key or path in OpenBao, or by adding a
  `[[infisical.mounts]]` folder mapping.

## 4. Migrate and verify

```sh
diegops kv migrate dpinto-config dpinto-work <other-mounts…>
# every line must be "OK"; the command exits non-zero on any MISMATCH
diegops kv migrate dpinto-config dpinto-work <other-mounts…> --dry-run
# second run: 0 to create, 0 to update (idempotent)
```

Rate limits on the Free tier are about 60 reads per minute. The client backs off on
HTTP 429, so a large run takes a few minutes. Re-running it is safe.

## 5. Switch the workstation over

```sh
diegops files status        # now reads Infisical; expect everything "in sync"
diegops files pull          # on a scratch machine or VM first
diegops env apply           # .env files from Infisical
gpg --list-secret-keys; ssh-add -l; sops -d <some file>   # smoke test the dpinto-work material
```

## 6. Delete from OpenBao (only after steps 4 and 5 pass)

1. Take an offline, encrypted backup first, because the Free tier has no secret
   versioning:
   `bao kv get -ns=personal -format=json …`, piped into `age -r <your key>`, and
   written to removable media.
2. Wait a few days of normal use on Infisical.
3. Run `bao kv metadata delete -ns=personal <mount>/<path>` for each path, or
   `bao secrets disable -ns=personal <mount>` per mount. Then remove the personal
   policies and roles, and the namespace itself once it is empty.
4. Remove the `[vault]` table from `config.toml`, unless you still use it for CaDi.

## Rollback

Nothing on the OpenBao side changes until step 6. Set `backend = "vault"` in
`config.toml`, or export `DIEGOPS_SECRETS_BACKEND=vault`, to go back immediately.
