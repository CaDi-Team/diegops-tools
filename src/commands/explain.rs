//! `diegops explain [term]`: a built-in glossary in plain words, so a new
//! user can understand help and error messages without leaving the terminal.

/// One glossary entry.
pub struct Term {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub summary: &'static str,
    pub body: &'static str,
    pub link: &'static str,
}

const README: &str = "https://github.com/CaDi-Team/diegops-tools#readme";
const BACKENDS_DOC: &str =
    "https://github.com/CaDi-Team/diegops-tools/blob/develop/docs/secrets-backends.md";

pub const GLOSSARY: &[Term] = &[
    Term {
        name: "backend",
        aliases: &["secrets-backend", "secret-store"],
        summary: "where diegops keeps your secrets (Infisical by default)",
        body: "A secrets backend is an online safe for passwords, tokens and keys.\n\
               diegops reads and writes secrets there instead of in git.\n\
               Infisical is the default; Vault or OpenBao can be used instead.\n\
               Pick one with --backend, $DIEGOPS_SECRETS_BACKEND or config.toml.\n\
               Check yours: diegops kv backend",
        link: BACKENDS_DOC,
    },
    Term {
        name: "infisical",
        aliases: &["infisical-cloud"],
        summary: "the default secrets backend (a hosted secrets manager)",
        body: "Infisical is a secrets manager with a web app (app.infisical.com).\n\
               You create a project there; diegops logs in with a machine identity\n\
               (Client ID + Client Secret) and reads/writes that project's secrets.\n\
               Config: [infisical] in ~/.config/diegops/config.toml.\n\
               See also: diegops explain machine-identity",
        link: "https://infisical.com/docs/documentation/getting-started/introduction",
    },
    Term {
        name: "machine-identity",
        aliases: &["identity", "universal-auth", "client-id", "client-secret"],
        summary: "the login diegops uses for Infisical (Client ID + Client Secret)",
        body: "A machine identity is an Infisical account for a program, not a person\n\
               (like a service account). With Universal Auth it has a Client ID and a\n\
               Client Secret. Create it in Infisical, give it access to your project,\n\
               then put client_id and client_secret_file (a file with mode 0600)\n\
               under [infisical] in config.toml. Never commit the Client Secret.",
        link: "https://infisical.com/docs/documentation/platform/identities/universal-auth",
    },
    Term {
        name: "vault",
        aliases: &["openbao", "bao", "hashicorp-vault"],
        summary: "an optional self-hosted secrets backend (Vault or OpenBao)",
        body: "HashiCorp Vault and its open-source fork OpenBao are secrets servers you\n\
               run yourself. diegops supports them as an alternative to Infisical.\n\
               You need an address (VAULT_ADDR) and a token (bao login / VAULT_TOKEN).\n\
               Not to be confused with 'diegops env' (formerly 'diegops vault'), which\n\
               writes .env files with any backend.",
        link: BACKENDS_DOC,
    },
    Term {
        name: "kv",
        aliases: &["secret-path", "path", "key-value"],
        summary: "a secret path holding KEY=VALUE pairs, e.g. dpinto-config/kube",
        body: "Secrets are stored as key/value pairs at a path: <mount>/<path>.\n\
               Example: dpinto-config/kube holds the key 'config'.\n\
               diegops kv list <mount>     list paths\n\
               diegops kv get <path>       key names (values only with --key)\n\
               diegops kv put <path> KEY=@file",
        link: BACKENDS_DOC,
    },
    Term {
        name: "mount",
        aliases: &["project", "project-id"],
        summary: "the first part of a secret path; maps to an Infisical project",
        body: "The mount is the first segment of a secret path (dpinto-config in\n\
               dpinto-config/kube). With Vault it is a KV engine; with Infisical it\n\
               maps to a project via [infisical] project_id or [[infisical.mounts]]\n\
               in config.toml.",
        link: BACKENDS_DOC,
    },
    Term {
        name: "pat",
        aliases: &["github-token", "token", "personal-access-token"],
        summary: "a GitHub personal access token, used by sync and repo clone",
        body: "A PAT is a password-like token for the GitHub API.\n\
               Create one at GitHub > Settings > Developer settings > Personal access\n\
               tokens (fine-grained: access to your diegops sync repo, Contents\n\
               read/write). Store it with 'diegops auth gh login'; check it with\n\
               'diegops auth gh whoami'. Treat it like a password.",
        link: "https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens",
    },
    Term {
        name: "env-file",
        aliases: &[".env", "dotenv", "env"],
        summary: "a project .env file that diegops writes from the backend",
        body: "Many apps read settings from a .env file in the project folder.\n\
               ~/.diegops/repo-vault.yaml says which secret path goes to which\n\
               folder; 'diegops env apply' writes the files (overwriting them).\n\
               .env files must never be committed to git.",
        link: README,
    },
    Term {
        name: "secret-files",
        aliases: &["secrets", "files"],
        summary: "local files like ~/.ssh and ~/.kube backed up to the backend",
        body: "Some secrets are whole files (SSH keys, kubeconfigs).\n\
               ~/.diegops/secrets.yaml lists the folders; 'diegops files push'\n\
               uploads them and 'diegops files pull' restores them on another\n\
               machine. 'diegops files status' compares both sides.",
        link: README,
    },
    Term {
        name: "workspace",
        aliases: &["repos", "repo"],
        summary: "your git repos cloned to known folders (repos.yaml)",
        body: "~/.diegops/repos.yaml lists repos and where they go.\n\
               'diegops repo apply' clones the missing ones (existing clones are\n\
               left alone); 'diegops repo list-diff' shows what is missing.",
        link: README,
    },
    Term {
        name: "sync",
        aliases: &["cloud-sync", "config-sync"],
        summary: "~/.diegops config mirrored to a private GitHub repo",
        body: "Your diegops YAML files are copied to a private repo named\n\
               diegops-<github-user>-memory so another machine can pull them.\n\
               Needs a GitHub token (diegops explain pat).\n\
               diegops sync push | pull | status",
        link: README,
    },
    Term {
        name: "config",
        aliases: &["config-files", "config.toml", "yaml"],
        summary: "where diegops settings live (config.toml + ~/.diegops/*.yaml)",
        body: "~/.config/diegops/config.toml   secrets backend settings\n\
               ~/.diegops/repos.yaml           repos to clone\n\
               ~/.diegops/secrets.yaml         secret files to back up\n\
               ~/.diegops/repo-vault.yaml      .env files to write\n\
               ~/.diegops/agents.yaml          AI agents to back up\n\
               Each '<command> init' writes a sample. Env vars: diegops --help",
        link: README,
    },
    Term {
        name: "agents",
        aliases: &["agent", "hermes", "profile"],
        summary: "AI agent setups (Hermes profiles) backed up to git",
        body: "diegops can back up an AI agent's definition (instructions, skills,\n\
               settings) to a git repo and its .env to the backend, then restore\n\
               it on another machine. Today: Hermes ('diegops agents hermes').\n\
               A profile is one named agent setup.",
        link: README,
    },
    Term {
        name: "tool",
        aliases: &["passthrough", "managed-tool"],
        summary: "a DevOps CLI that diegops installs and runs for you",
        body: "diegops downloads tools like gh, kubectl, terraform or jq into\n\
               ~/.diegops/bin and runs them as 'diegops <tool> <args>'.\n\
               diegops tool list | install <name> | update | remove <name>",
        link: README,
    },
    Term {
        name: "kubeconfig",
        aliases: &["kube", "kubectl-config"],
        summary: "the file that tells kubectl how to reach a Kubernetes cluster",
        body: "A kubeconfig holds a cluster address and credentials.\n\
               diegops backs up ~/.kube with 'secrets push' and the managed shell\n\
               config sets KUBECONFIG to every file in ~/.kube.",
        link: "https://kubernetes.io/docs/concepts/configuration/organize-cluster-access-kubeconfig/",
    },
];

fn norm(s: &str) -> String {
    s.trim().to_lowercase().replace([' ', '_'], "-")
}

/// Finds a term by name or alias (case/space/underscore insensitive).
pub fn find(term: &str) -> Option<&'static Term> {
    let t = norm(term);
    GLOSSARY
        .iter()
        .find(|e| e.name == t || e.aliases.iter().any(|a| norm(a) == t))
}

pub fn run(term: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    match term {
        None => {
            let w = GLOSSARY.iter().map(|e| e.name.len()).max().unwrap_or(0);
            println!("Terms (diegops explain <term>):\n");
            for e in GLOSSARY {
                println!("  {:<w$}  {}", e.name, e.summary);
            }
        }
        Some(t) => match find(t) {
            Some(e) => {
                println!("{}: {}\n", e.name, e.summary);
                println!("{}\n", e.body);
                println!("More: {}", e.link);
            }
            None => {
                let names: Vec<String> = GLOSSARY
                    .iter()
                    .flat_map(|e| std::iter::once(e.name).chain(e.aliases.iter().copied()))
                    .map(String::from)
                    .collect();
                eprintln!("error: unknown term '{t}'");
                if let Some(s) = crate::help::suggest(&norm(t), &names) {
                    eprintln!("  hint: did you mean '{s}'?");
                }
                eprintln!("  hint: run 'diegops explain' to list all terms");
                std::process::exit(2);
            }
        },
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_aliases_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for e in GLOSSARY {
            for n in std::iter::once(e.name).chain(e.aliases.iter().copied()) {
                assert!(seen.insert(norm(n)), "duplicate term/alias '{n}'");
            }
        }
    }

    #[test]
    fn entries_are_short_and_linked() {
        for e in GLOSSARY {
            assert!(e.body.lines().count() <= 6, "{} too long", e.name);
            assert!(e.link.starts_with("https://"), "{} link", e.name);
        }
    }

    #[test]
    fn every_explain_reference_in_source_exists() {
        // Any "diegops explain <term>" written in help or errors must resolve.
        let sources = [
            include_str!("../main.rs"),
            include_str!("../examples.rs"),
            include_str!("../secrets_backend/mod.rs"),
            include_str!("../secrets_backend/config.rs"),
            include_str!("explain.rs"),
        ];
        for src in sources {
            for (i, _) in src.match_indices("diegops explain ") {
                let rest = &src[i + "diegops explain ".len()..];
                let term: String = rest
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
                    .collect();
                if term.is_empty() {
                    continue;
                }
                assert!(
                    find(&term).is_some(),
                    "'diegops explain {term}' has no entry"
                );
            }
        }
    }
}
