//! Global interaction rules (ADR 0001 D3/D4): when diegops may prompt,
//! how confirmations behave without a terminal, and which commands honour
//! `--dry-run`.

use std::sync::OnceLock;

/// Global flags shared by every command.
#[derive(Debug, Default, Clone, Copy)]
pub struct Flags {
    pub yes: bool,
    pub no_input: bool,
    pub dry_run: bool,
}

static FLAGS: OnceLock<Flags> = OnceLock::new();

/// Records the global flags once, at startup.
pub fn set(flags: Flags) {
    let _ = FLAGS.set(flags);
}

/// The global flags (defaults if not set, e.g. in unit tests).
pub fn flags() -> Flags {
    FLAGS.get().copied().unwrap_or_default()
}

/// What a confirmation does.
#[derive(Debug, PartialEq, Eq)]
pub enum Confirm {
    /// `--yes`: proceed without asking.
    Proceed,
    /// Interactive: ask the user.
    Ask,
    /// No terminal and no `--yes`: refuse (exit 2, hint `--yes`).
    Refuse,
}

/// May diegops prompt? Needs a terminal on stdin and stderr, and none of
/// `--no-input`, `$CI`, `$DIEGOPS_NO_INPUT`.
pub fn can_prompt(
    stdin_tty: bool,
    stderr_tty: bool,
    no_input: bool,
    env: &dyn Fn(&str) -> Option<String>,
) -> bool {
    let set = |k: &str| env(k).is_some_and(|v| !v.is_empty() && v != "0" && v != "false");
    stdin_tty && stderr_tty && !no_input && !set("CI") && !set("DIEGOPS_NO_INPUT")
}

/// The confirmation rule: `--yes` wins, then a terminal asks, else refuse.
pub fn confirm_rule(yes: bool, can_prompt: bool) -> Confirm {
    if yes {
        Confirm::Proceed
    } else if can_prompt {
        Confirm::Ask
    } else {
        Confirm::Refuse
    }
}

/// Commands (space-joined path) that implement `--dry-run`. Any other
/// command given `--dry-run` is rejected instead of silently writing.
pub const DRY_RUN_COMMANDS: &[&str] = &[
    "shell init",
    "kv migrate",
    "env apply",
    "files pull",
    "files push",
    "agents hermes restore",
];

/// `Err` (usage) if `--dry-run` was given to a command that cannot honour it.
pub fn check_dry_run(dry_run: bool, path: &str) -> Result<(), String> {
    if !dry_run || DRY_RUN_COMMANDS.contains(&path) {
        return Ok(());
    }
    Err(format!(
        "error: --dry-run is not supported by 'diegops {path}' yet\n  \
         hint: supported: {}",
        DRY_RUN_COMMANDS
            .iter()
            .map(|c| format!("'diegops {c}'"))
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

/// Asks `question` (default yes) or applies the rule; `Err` when refused.
pub fn confirm(question: &str) -> Result<bool, Box<dyn std::error::Error>> {
    use std::io::{BufRead, IsTerminal, Write};
    let f = flags();
    let prompt_ok = can_prompt(
        std::io::stdin().is_terminal(),
        std::io::stderr().is_terminal(),
        f.no_input,
        &|k| std::env::var(k).ok(),
    );
    match confirm_rule(f.yes, prompt_ok) {
        Confirm::Proceed => Ok(true),
        Confirm::Refuse => Err(format!(
            "confirmation needed: {question}\n  \
             hint: no terminal to ask in; re-run with --yes to accept, or --dry-run to preview"
        )
        .into()),
        Confirm::Ask => {
            eprint!("\n  {question} [Y/n] ");
            let _ = std::io::stderr().flush();
            let mut answer = String::new();
            if std::io::stdin().lock().read_line(&mut answer).is_err() {
                return Ok(false);
            }
            Ok(!matches!(answer.trim().to_lowercase().as_str(), "n" | "no"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env_of(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
        move |k| {
            pairs
                .iter()
                .find(|(n, _)| *n == k)
                .map(|(_, v)| v.to_string())
        }
    }

    #[test]
    fn prompts_only_with_a_terminal_and_no_opt_out() {
        let none = env_of(&[]);
        assert!(can_prompt(true, true, false, &none));
        assert!(!can_prompt(false, true, false, &none), "stdin piped");
        assert!(!can_prompt(true, false, false, &none), "stderr redirected");
        assert!(!can_prompt(true, true, true, &none), "--no-input");
        assert!(!can_prompt(true, true, false, &env_of(&[("CI", "true")])));
        assert!(!can_prompt(
            true,
            true,
            false,
            &env_of(&[("DIEGOPS_NO_INPUT", "1")])
        ));
        assert!(can_prompt(true, true, false, &env_of(&[("CI", "false")])));
    }

    #[test]
    fn confirmation_without_terminal_is_refused_not_assumed() {
        assert_eq!(confirm_rule(true, false), Confirm::Proceed);
        assert_eq!(confirm_rule(true, true), Confirm::Proceed);
        assert_eq!(confirm_rule(false, true), Confirm::Ask);
        assert_eq!(confirm_rule(false, false), Confirm::Refuse);
    }

    #[test]
    fn dry_run_is_rejected_where_not_implemented() {
        assert!(check_dry_run(false, "env list").is_ok());
        assert!(check_dry_run(true, "shell init").is_ok());
        assert!(check_dry_run(true, "kv migrate").is_ok());
        let e = check_dry_run(true, "env list").unwrap_err();
        assert!(
            e.starts_with("error: --dry-run is not supported by 'diegops env list'"),
            "{e}"
        );
        assert!(e.contains("'diegops shell init'"), "{e}");
    }
}
