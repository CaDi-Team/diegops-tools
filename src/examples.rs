//! Examples shown at the end of `--help` for every command, plus the
//! ENVIRONMENT section. Kept in one table so a test can enforce coverage.

use clap::Command;

/// `(command path, examples)`; the path is space-separated under `diegops`.
pub const EXAMPLES: &[(&str, &str)] = &[
    ("version", "diegops version"),
    ("init", "diegops init                 # guided first run (prompts, validates, reviews)\ndiegops init --dry-run       # stop at the review screen\nINFISICAL_UNIVERSAL_AUTH_CLIENT_SECRET=... diegops init --no-input --project-id P --client-id C --skip-github"),
    ("doctor", "diegops doctor              # check everything, with a hint per problem\ndiegops doctor --offline    # no network calls\ndiegops doctor --json | jq '.checks[] | select(.status==\"fail\")'"),
    ("explain", "diegops explain                     # list all terms\ndiegops explain machine-identity    # what it is and where to get one"),
    ("completion", "diegops completion zsh > ~/.zfunc/_diegops     # or: source <(diegops completion zsh)\ndiegops completion bash > ~/.local/share/bash-completion/completions/diegops\ndiegops completion fish > ~/.config/fish/completions/diegops.fish\ndiegops man | man -l -                              # read the man page"),
    ("update", "diegops update              # replace this binary with the latest release\nsudo diegops update         # if installed in a system path"),
    ("cadi", "diegops cadi"),
    ("bootstrap", "diegops bootstrap           # new machine: auth check, pull config, clone repos, secrets"),
    ("all", "diegops all pull            # get everything onto this machine\ndiegops all push            # save everything from this machine"),
    ("all pull", "diegops all pull"),
    ("all push", "diegops all push"),
    ("auth", "diegops auth status\ndiegops auth gh whoami"),
    ("auth gh", "diegops auth gh login\ndiegops auth gh whoami"),
    ("auth gh login", "diegops auth gh login                       # hidden prompt\ngh auth token | diegops auth gh login --with-token\ndiegops auth gh login --from-gh             # reuse the GitHub CLI login"),
    ("auth gh logout", "diegops auth gh logout"),
    ("auth gh whoami", "diegops auth gh whoami      # which GitHub user the stored token belongs to"),
    ("auth status", "diegops auth status"),
    ("auth logout", "diegops auth logout         # remove every stored token"),
    ("shell", "diegops shell init"),
    ("shell init", "diegops shell init --dry-run   # see what would change\ndiegops shell init             # install zsh + oh-my-zsh + plugins, write config\ndiegops shell init --yes       # no questions (scripts, containers)"),
    ("env", "diegops env init          # sample ~/.diegops/repo-vault.yaml\ndiegops env apply         # write every .env from the secrets backend\ndiegops env list-diff     # which .env files are missing"),
    ("env apply", "diegops env apply\ndiegops env apply --path ~/github/acme    # only targets under this folder"),
    ("env list", "diegops env list"),
    ("env list-diff", "diegops env list-diff"),
    ("env init", "diegops env init"),
    ("files", "diegops files init        # sample ~/.diegops/secrets.yaml\ndiegops files push        # upload ~/.ssh, ~/.kube, ... to the backend\ndiegops files pull        # restore them on another machine"),
    ("files push", "diegops files push\ndiegops files push --path ~/.kube         # only this folder"),
    ("files pull", "diegops files pull\ndiegops files pull --path ~/.ssh"),
    ("files status", "diegops files status      # local vs backend, per folder"),
    ("files init", "diegops files init"),
    ("files update", "diegops files update      # register CLAUDE.md / AGENTS.md found in your repos"),
    ("kv", "diegops kv backend          # which backend and config are in use\ndiegops kv list dpinto-config\ndiegops kv get dpinto-config/kube"),
    ("kv backend", "diegops kv backend\ndiegops kv backend --backend vault"),
    ("kv get", "diegops kv get dpinto-config/kube                  # key names only\ndiegops kv get dpinto-config/kube --key config     # one raw value"),
    ("kv list", "diegops kv list dpinto-config"),
    ("kv put", "diegops kv put dpinto-config/kube config=@$HOME/.kube/config\necho -n \"$TOKEN\" | diegops kv put acme/ci GH_TOKEN=-    # value from stdin"),
    ("kv migrate", "diegops kv migrate dpinto-config --dry-run         # plan only\ndiegops kv migrate dpinto-config dpinto-work --from vault --to infisical"),
    ("repo", "diegops repo init           # sample ~/.diegops/repos.yaml\ndiegops repo apply          # clone everything missing\ndiegops repo list-diff"),
    ("repo apply", "diegops repo apply\ndiegops repo apply --path ~/github/acme"),
    ("repo list", "diegops repo list"),
    ("repo list-diff", "diegops repo list-diff"),
    ("repo init", "diegops repo init"),
    ("sync", "diegops sync push           # upload ~/.diegops/*.yaml to your private GitHub repo\ndiegops sync pull\ndiegops sync status"),
    ("sync push", "diegops sync push\nDIEGOPS_PROFILE=work diegops sync push"),
    ("sync pull", "diegops sync pull"),
    ("sync status", "diegops sync status"),
    ("devtools", "diegops devtools git set --name \"Ana Pérez\" --email ana@example.com\ndiegops devtools gpg set\ndiegops devtools ssh create --name laptop"),
    ("devtools git", "diegops devtools git set --name \"Ana Pérez\" --email ana@example.com"),
    ("devtools git set", "diegops devtools git set --name \"Ana Pérez\" --email ana@example.com"),
    ("devtools gpg", "diegops devtools gpg init --name \"Ana Pérez\" --email ana@example.com\ndiegops devtools gpg set"),
    ("devtools gpg init", "diegops devtools gpg init --name \"Ana Pérez\" --email ana@example.com"),
    ("devtools gpg set", "diegops devtools gpg set    # needs gh logged in (diegops gh auth login)"),
    ("devtools gpg restart", "diegops devtools gpg restart"),
    ("devtools ssh", "diegops devtools ssh list\ndiegops devtools ssh create --name laptop --email ana@example.com"),
    ("devtools ssh list", "diegops devtools ssh list"),
    ("devtools ssh config", "diegops devtools ssh config"),
    ("devtools ssh create", "diegops devtools ssh create --name laptop --email ana@example.com\ndiegops devtools ssh create --name legacy --type rsa"),
    ("tool", "diegops tool list\ndiegops tool install kubectl\ndiegops kubectl get pods    # run a managed tool"),
    ("tool list", "diegops tool list"),
    ("tool install", "diegops tool install jq\ndiegops tool install terraform"),
    ("tool update", "diegops tool update         # every installed tool\ndiegops tool update gh"),
    ("tool remove", "diegops tool remove trivy"),
    ("ktool", "diegops ktool --help"),
    ("agents", "diegops agents init         # sample ~/.diegops/agents.yaml\ndiegops agents hermes backup\ndiegops agents status"),
    ("agents hermes", "diegops agents hermes status\ndiegops agents hermes backup\ndiegops agents hermes restore --to /tmp/check   # dry run into a folder"),
    ("agents hermes backup", "diegops agents hermes backup\ndiegops agents hermes backup --profile work"),
    ("agents hermes restore", "diegops agents hermes restore --to /tmp/check   # safe: writes under /tmp/check/<profile>\ndiegops agents hermes restore --profile work\ndiegops agents hermes restore --force           # overwrite, keeps *.pre-restore copies"),
    ("agents hermes status", "diegops agents hermes status"),
    ("agents hermes memory", "diegops agents hermes memory review\ndiegops agents hermes memory push"),
    ("agents hermes memory review", "diegops agents hermes memory review\ndiegops agents hermes memory review --profile work"),
    ("agents hermes memory push", "diegops agents hermes memory push --profile work"),
    ("agents hermes secrets", "diegops agents hermes secrets push"),
    ("agents hermes secrets push", "diegops agents hermes secrets push            # each profile's .env, values never printed\ndiegops agents hermes secrets push --profile work"),
    ("agents status", "diegops agents status"),
    ("agents install-timer", "diegops agents install-timer"),
    ("agents init", "diegops agents init"),
];

/// ENVIRONMENT section appended to the top-level `--help`.
pub const ENVIRONMENT: &str = "ENVIRONMENT:
  DIEGOPS_CONFIG                          config.toml path (default ~/.config/diegops/config.toml)
  DIEGOPS_SECRETS_BACKEND                 infisical | vault (overrides config.toml)
  DIEGOPS_REPOS_CONFIG                    repos.yaml path (default ~/.diegops/repos.yaml)
  DIEGOPS_SECRETS_CONFIG                  secrets.yaml path (default ~/.diegops/secrets.yaml)
  DIEGOPS_VAULT_CONFIG                    repo-vault.yaml path (default ~/.diegops/repo-vault.yaml)
  DIEGOPS_AGENTS_CONFIG                   agents.yaml path (default ~/.diegops/agents.yaml)
  DIEGOPS_PROFILE                         sync profile name (separate sync repo per profile)
  GITHUB_TOKEN, GH_TOKEN                  GitHub token; overrides the one stored by 'auth gh login'
  DIEGOPS_NO_GH_CLI                       do not fall back to 'gh auth token'
  CI, DIEGOPS_NO_INPUT                    never prompt (same as --no-input)
  INFISICAL_TOKEN                         Infisical access token (skips machine identity login)
  INFISICAL_UNIVERSAL_AUTH_CLIENT_ID      Infisical machine identity client id
  INFISICAL_UNIVERSAL_AUTH_CLIENT_SECRET  Infisical machine identity client secret
  INFISICAL_API_URL                       Infisical URL (default https://app.infisical.com)
  VAULT_ADDR, VAULT_TOKEN                 Vault/OpenBao address and token
  VAULT_NAMESPACE, VAULT_CACERT           Vault namespace and CA certificate
  NO_COLOR                                disable colored output
  INFISICAL_* and VAULT_* override config.toml; --config flags override DIEGOPS_*_CONFIG.";

/// Attaches the examples to every command in the tree.
pub fn apply(cmd: Command) -> Command {
    let top = cmd
        .get_after_long_help()
        .map(|h| h.to_string())
        .unwrap_or_default();
    let mut cmd = cmd.after_long_help(format!("{top}\n\n{ENVIRONMENT}"));
    for (path, ex) in EXAMPLES {
        let segs: Vec<&str> = path.split(' ').collect();
        cmd = attach(cmd, &segs, ex);
    }
    cmd
}

fn attach(cmd: Command, segs: &[&str], ex: &str) -> Command {
    let (first, rest) = (segs[0], &segs[1..]);
    cmd.mut_subcommand(first, |sub| {
        if rest.is_empty() {
            let body = ex
                .lines()
                .map(|l| format!("  {l}"))
                .collect::<Vec<_>>()
                .join("\n");
            let prev = sub
                .get_after_long_help()
                .map(|h| format!("{h}\n\n"))
                .unwrap_or_default();
            sub.after_long_help(format!("{prev}Examples:\n{body}"))
        } else {
            attach(sub, rest, ex)
        }
    })
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    fn paths(cmd: &clap::Command, prefix: &str, out: &mut Vec<String>) {
        for s in cmd.get_subcommands().filter(|s| !s.is_hide_set()) {
            if s.get_name() == "help" {
                continue;
            }
            let p = if prefix.is_empty() {
                s.get_name().to_string()
            } else {
                format!("{prefix} {}", s.get_name())
            };
            out.push(p.clone());
            paths(s, &p, out);
        }
    }

    #[test]
    fn every_command_has_examples() {
        let cmd = super::apply(crate::Cli::command());
        let mut all = Vec::new();
        paths(&cmd, "", &mut all);
        let mut missing = Vec::new();
        for p in &all {
            let mut c = &cmd;
            for seg in p.split(' ') {
                c = c.find_subcommand(seg).unwrap();
            }
            let help = c
                .get_after_long_help()
                .map(|h| h.to_string())
                .unwrap_or_default();
            if !help.contains("Examples:\n  diegops ") {
                missing.push(p.clone());
            }
        }
        assert!(missing.is_empty(), "no Examples: {missing:?}");
    }

    #[test]
    fn every_example_path_exists() {
        let cmd = crate::Cli::command();
        for (p, _) in super::EXAMPLES {
            let mut c = &cmd;
            for seg in p.split(' ') {
                c = c
                    .find_subcommand(seg)
                    .unwrap_or_else(|| panic!("unknown path '{p}'"));
            }
        }
    }

    #[test]
    fn env_vars_read_by_code_are_documented() {
        let help = super::apply(crate::Cli::command())
            .get_after_long_help()
            .map(|h| h.to_string())
            .unwrap_or_default();
        for var in [
            "DIEGOPS_CONFIG",
            "DIEGOPS_SECRETS_BACKEND",
            "DIEGOPS_REPOS_CONFIG",
            "DIEGOPS_SECRETS_CONFIG",
            "DIEGOPS_VAULT_CONFIG",
            "DIEGOPS_AGENTS_CONFIG",
            "DIEGOPS_PROFILE",
            "GITHUB_TOKEN",
            "GH_TOKEN",
            "DIEGOPS_NO_INPUT",
            "DIEGOPS_NO_GH_CLI",
            "INFISICAL_TOKEN",
            "INFISICAL_UNIVERSAL_AUTH_CLIENT_ID",
            "INFISICAL_UNIVERSAL_AUTH_CLIENT_SECRET",
            "INFISICAL_API_URL",
            "VAULT_ADDR",
            "VAULT_TOKEN",
            "VAULT_NAMESPACE",
            "VAULT_CACERT",
            "NO_COLOR",
        ] {
            assert!(help.contains(var), "{var} not in ENVIRONMENT section");
        }
    }

    #[test]
    fn help_has_no_repo_relative_doc_paths() {
        let mut texts = Vec::new();
        fn collect(c: &clap::Command, t: &mut Vec<String>) {
            for s in [c.get_long_about(), c.get_after_long_help()]
                .into_iter()
                .flatten()
            {
                t.push(s.to_string());
            }
            for s in c.get_subcommands() {
                collect(s, t);
            }
        }
        collect(&crate::Cli::command(), &mut texts);
        for t in texts {
            assert!(!t.contains("see docs/") && !t.contains("See docs/"), "{t}");
        }
    }

    #[test]
    fn every_argument_has_a_description() {
        fn walk(c: &clap::Command, p: &str) {
            for a in c.get_arguments() {
                if a.is_hide_set() || ["help", "version"].contains(&a.get_id().as_str()) {
                    continue;
                }
                assert!(a.get_help().is_some(), "{p}: --{} has no help", a.get_id());
            }
            for s in c.get_subcommands() {
                walk(s, &format!("{p} {}", s.get_name()));
            }
        }
        walk(&crate::Cli::command(), "diegops");
    }
}
