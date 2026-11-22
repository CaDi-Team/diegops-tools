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
    pub quiet: bool,
    pub verbose: bool,
    pub json: bool,
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

/// Progress lines (stderr) unless `--quiet`. Errors, warnings, hints and
/// results are never hidden.
pub fn show_progress(f: Flags) -> bool {
    !f.quiet
}

/// Diagnostic lines (where config/backend/token come from) with `--verbose`.
pub fn show_debug(f: Flags) -> bool {
    f.verbose
}

/// Progress line on stderr, hidden by `--quiet`.
#[macro_export]
macro_rules! progress {
    ($($t:tt)*) => {
        if $crate::ui::show_progress($crate::ui::flags()) {
            eprintln!($($t)*);
        }
    };
}

/// Diagnostic line on stderr (`diegops: ...`), shown only with `--verbose`.
/// Never pass secret values.
#[macro_export]
macro_rules! debug {
    ($($t:tt)*) => {
        if $crate::ui::show_debug($crate::ui::flags()) {
            eprintln!("diegops: {}", format!($($t)*));
        }
    };
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

/// Read commands with a documented `--json` schema (ADR 0001 D4; README
/// "JSON output"). Names only, never secret values.
pub const JSON_COMMANDS: &[&str] = &[
    "doctor",
    "files status",
    "kv list",
    "kv get",
    "repo list",
    "repo list-diff",
    "tool list",
    "agents status",
    "agents hermes status",
];

/// `Err` (usage) if `--json` was given to a command without a JSON schema.
pub fn check_json(json: bool, path: &str) -> Result<(), String> {
    if !json || JSON_COMMANDS.contains(&path) {
        return Ok(());
    }
    Err(format!(
        "error: --json is not supported by 'diegops {path}'\n  \
         hint: supported: {}",
        JSON_COMMANDS
            .iter()
            .map(|c| format!("'diegops {c}'"))
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

/// Prints `v` as pretty JSON on stdout.
pub fn print_json(v: &impl serde::Serialize) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", serde_json::to_string_pretty(v)?);
    Ok(())
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
    #[test]
    fn json_only_on_read_commands() {
        assert!(check_json(true, "repo list").is_ok());
        assert!(check_json(false, "env apply").is_ok());
        let e = check_json(true, "env apply").unwrap_err();
        assert!(
            e.contains("--json is not supported by 'diegops env apply'"),
            "{e}"
        );
    }

    #[test]
    fn quiet_hides_progress_verbose_shows_debug() {
        let f = |quiet, verbose| Flags {
            quiet,
            verbose,
            ..Flags::default()
        };
        assert!(show_progress(f(false, false)) && !show_debug(f(false, false)));
        assert!(!show_progress(f(true, false)));
        assert!(show_progress(f(false, true)) && show_debug(f(false, true)));
    }

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
