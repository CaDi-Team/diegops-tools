//! Help discoverability: grouped top-level help, `diegops help <cmd…>`,
//! and "did you mean" suggestions for unknown commands and tools.

use clap::Command;

/// Top-level commands grouped by what a user is trying to do.
/// Every visible subcommand must appear exactly once (enforced by a test).
pub const GROUPS: &[(&str, &[&str])] = &[
    (
        "Get started",
        &[
            "init",
            "bootstrap",
            "all",
            "auth",
            "shell",
            "doctor",
            "explain",
        ],
    ),
    ("Secrets", &["env", "files", "kv"]),
    ("Workspace", &["repo", "sync", "devtools"]),
    ("Tools", &["tool", "ktool"]),
    ("AI agents", &["agents"]),
    (
        "About",
        &["help", "completion", "version", "update", "cadi"],
    ),
];

/// Renders the grouped command list used in the top-level help template.
pub fn grouped_commands(cmd: &Command) -> String {
    let width = GROUPS
        .iter()
        .flat_map(|(_, names)| names.iter())
        .map(|n| n.len())
        .max()
        .unwrap_or(0);
    let mut out = String::new();
    for (heading, names) in GROUPS {
        out.push_str(&format!("{heading}:\n"));
        for name in *names {
            let about = cmd
                .find_subcommand(name)
                .and_then(|s| s.get_about())
                .map(|a| a.to_string())
                .unwrap_or_default();
            out.push_str(&format!("  {name:<width$}  {about}\n"));
        }
        out.push('\n');
    }
    out
}

/// Top-level help template with grouped commands instead of a flat list.
pub fn template(cmd: &Command) -> String {
    format!(
        "{{before-help}}{{about-with-newline}}\n{{usage-heading}} {{usage}}\n\n{}Options:\n{{options}}{{after-help}}",
        grouped_commands(cmd)
    )
}

/// Validates a `help` path (e.g. `["repo", "apply"]`) against the command tree.
/// Returns the first unknown segment and the valid names at that level.
pub fn check_path(cmd: &Command, path: &[String]) -> Result<(), (String, Vec<String>, String)> {
    let mut cur = cmd;
    for (i, seg) in path.iter().enumerate() {
        match cur.find_subcommand(seg) {
            Some(next) => cur = next,
            None => {
                let help_cmd = std::iter::once("diegops help")
                    .chain(path[..i].iter().map(String::as_str))
                    .collect::<Vec<_>>()
                    .join(" ");
                return Err((seg.clone(), visible_names(cur), help_cmd));
            }
        }
    }
    Ok(())
}

/// Names of the visible subcommands (plus aliases) of `cmd`.
pub fn visible_names(cmd: &Command) -> Vec<String> {
    cmd.get_subcommands()
        .filter(|s| !s.is_hide_set())
        .flat_map(|s| {
            std::iter::once(s.get_name().to_string())
                .chain(s.get_visible_aliases().map(str::to_string))
        })
        .collect()
}

/// Closest candidate to `input`, if it is plausibly a typo.
pub fn suggest<'a>(input: &str, candidates: &'a [String]) -> Option<&'a str> {
    candidates
        .iter()
        .map(|c| (levenshtein(input, c), c))
        .filter(|(d, c)| *d <= 2 || (input.len() >= 3 && c.starts_with(input)))
        .min_by_key(|(d, _)| *d)
        .map(|(_, c)| c.as_str())
}

fn levenshtein(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != *cb);
            cur.push((prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}

/// Renamed top-level commands (old, new), ADR 0001 D1. Old names are hidden
/// aliases; typos of them suggest the new name.
pub const RENAMED: &[(&str, &str)] = &[("vault", "env"), ("secrets", "files")];

/// `error: unknown command '<x>'` with a suggestion or a pointer to help.
pub fn unknown_command(name: &str, candidates: &[String], help_cmd: &str) -> String {
    let mut msg = format!("error: unknown command '{name}'");
    let mut all = candidates.to_vec();
    all.extend(RENAMED.iter().map(|(old, _)| old.to_string()));
    let suggestion = suggest(name, &all).map(|s| {
        RENAMED
            .iter()
            .find(|(old, _)| *old == s)
            .map_or(s, |(_, new)| *new)
    });
    if let Some(s) = suggestion {
        msg.push_str(&format!("\n  hint: did you mean '{s}'?"));
    }
    msg.push_str(&format!(
        "\n  hint: run '{help_cmd}' to see available commands"
    ));
    msg
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn every_visible_command_is_in_exactly_one_group() {
        let cmd = crate::Cli::command();
        let grouped: Vec<&str> = GROUPS.iter().flat_map(|(_, n)| n.iter().copied()).collect();
        for name in cmd
            .get_subcommands()
            .filter(|s| !s.is_hide_set())
            .map(|s| s.get_name())
        {
            let n = grouped.iter().filter(|g| **g == name).count();
            assert_eq!(n, 1, "'{name}' must be in exactly one help group");
        }
        for g in &grouped {
            assert!(
                cmd.find_subcommand(g).is_some(),
                "group lists unknown '{g}'"
            );
        }
    }

    #[test]
    fn suggests_close_names_only() {
        let c: Vec<String> = ["secrets", "sync", "kubectl"].map(String::from).to_vec();
        assert_eq!(suggest("secret", &c), Some("secrets"));
        assert_eq!(suggest("kubctl", &c), Some("kubectl"));
        assert_eq!(suggest("zzzzzz", &c), None);
    }

    #[test]
    fn levenshtein_basics() {
        assert_eq!(levenshtein("", "abc"), 3);
        assert_eq!(levenshtein("kitten", "sitting"), 3);
        assert_eq!(levenshtein("same", "same"), 0);
    }
}
