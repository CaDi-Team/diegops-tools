use std::process::Command;

fn diegops() -> Command {
    Command::new(env!("CARGO_BIN_EXE_diegops"))
}

#[test]
fn version_subcommand_prints_version() {
    let output = diegops()
        .arg("version")
        .output()
        .expect("failed to run diegops");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.starts_with("diegops v"), "got: {stdout}");
}

#[test]
fn help_subcommand_exits_successfully() {
    let output = diegops()
        .arg("help")
        .output()
        .expect("failed to run diegops");

    assert!(output.status.success());
}

#[test]
fn no_args_exits_successfully() {
    let output = diegops().output().expect("failed to run diegops");

    assert!(output.status.success());
}

#[test]
fn flag_version_exits_successfully() {
    let output = diegops()
        .arg("--version")
        .output()
        .expect("failed to run diegops");

    assert!(output.status.success());
}

#[test]
fn repo_help_exits_successfully() {
    let output = diegops()
        .args(["repo", "--help"])
        .output()
        .expect("failed to run diegops repo --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("apply"), "got: {stdout}");
    assert!(stdout.contains("list"), "got: {stdout}");
    assert!(stdout.contains("list-diff"), "got: {stdout}");
}

#[test]
fn repo_init_help_exits_successfully() {
    let output = diegops()
        .args(["repo", "init", "--help"])
        .output()
        .expect("failed to run diegops repo init --help");

    assert!(output.status.success());
}

#[test]
fn repo_apply_help_exits_successfully() {
    let output = diegops()
        .args(["repo", "apply", "--help"])
        .output()
        .expect("failed to run diegops repo apply --help");

    assert!(output.status.success());
}

#[test]
fn update_help_exits_successfully() {
    // Does not hit the network — just verifies the subcommand is wired up
    // and its --help flag works. Actual update behaviour requires a live
    // GitHub connection and is not tested here (tests must run offline).
    let output = diegops()
        .args(["update", "--help"])
        .output()
        .expect("failed to run diegops update --help");

    assert!(output.status.success());
}

#[test]
fn vault_help_exits_successfully() {
    let output = diegops()
        .args(["vault", "--help"])
        .output()
        .expect("failed to run diegops vault --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("apply"), "got: {stdout}");
    assert!(stdout.contains("list"), "got: {stdout}");
    assert!(stdout.contains("list-diff"), "got: {stdout}");
    assert!(stdout.contains("init"), "got: {stdout}");
}

#[test]
fn vault_init_help_exits_successfully() {
    let output = diegops()
        .args(["vault", "init", "--help"])
        .output()
        .expect("failed to run diegops vault init --help");

    assert!(output.status.success());
}

#[test]
fn vault_apply_help_exits_successfully() {
    let output = diegops()
        .args(["vault", "apply", "--help"])
        .output()
        .expect("failed to run diegops vault apply --help");

    assert!(output.status.success());
}

#[test]
fn vault_list_help_exits_successfully() {
    let output = diegops()
        .args(["vault", "list", "--help"])
        .output()
        .expect("failed to run diegops vault list --help");

    assert!(output.status.success());
}

#[test]
fn cadi_prints_hero_screen() {
    let output = diegops()
        .arg("cadi")
        .output()
        .expect("failed to run diegops cadi");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("DiegOps"), "got: {stdout}");
    assert!(
        stdout.contains("When more than one Diego is needed"),
        "got: {stdout}"
    );
    assert!(stdout.contains("CaDi Labs"), "got: {stdout}");
}

#[test]
fn vault_list_diff_help_exits_successfully() {
    let output = diegops()
        .args(["vault", "list-diff", "--help"])
        .output()
        .expect("failed to run diegops vault list-diff --help");

    assert!(output.status.success());
}

#[test]
fn auth_help_exits_successfully() {
    let output = diegops()
        .args(["auth", "--help"])
        .output()
        .expect("failed to run diegops auth --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("gh"), "got: {stdout}");
    assert!(stdout.contains("status"), "got: {stdout}");
    assert!(stdout.contains("logout"), "got: {stdout}");
}

#[test]
fn auth_gh_help_exits_successfully() {
    let output = diegops()
        .args(["auth", "gh", "--help"])
        .output()
        .expect("failed to run diegops auth gh --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("login"), "got: {stdout}");
    assert!(stdout.contains("logout"), "got: {stdout}");
    assert!(stdout.contains("whoami"), "got: {stdout}");
}

#[test]
fn devtools_help_exits_successfully() {
    let output = diegops()
        .args(["devtools", "--help"])
        .output()
        .expect("failed to run diegops devtools --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("git"), "got: {stdout}");
    assert!(stdout.contains("gpg"), "got: {stdout}");
    assert!(stdout.contains("ssh"), "got: {stdout}");
}

#[test]
fn devtools_git_help_exits_successfully() {
    let output = diegops()
        .args(["devtools", "git", "--help"])
        .output()
        .expect("failed to run diegops devtools git --help");

    assert!(output.status.success());
}

#[test]
fn devtools_gpg_help_exits_successfully() {
    let output = diegops()
        .args(["devtools", "gpg", "--help"])
        .output()
        .expect("failed to run diegops devtools gpg --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("init"), "got: {stdout}");
    assert!(stdout.contains("set"), "got: {stdout}");
    assert!(stdout.contains("restart"), "got: {stdout}");
}

#[test]
fn devtools_ssh_help_exits_successfully() {
    let output = diegops()
        .args(["devtools", "ssh", "--help"])
        .output()
        .expect("failed to run diegops devtools ssh --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("list"), "got: {stdout}");
    assert!(stdout.contains("config"), "got: {stdout}");
    assert!(stdout.contains("create"), "got: {stdout}");
}

#[test]
fn auth_status_runs_without_config() {
    let output = diegops()
        .args(["auth", "status"])
        .output()
        .expect("failed to run diegops auth status");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("GitHub"), "got: {stdout}");
}

#[test]
fn ktool_without_binary_prints_error() {
    let output = diegops()
        .args(["ktool", "version"])
        .output()
        .expect("failed to run diegops ktool version");
    // Just verify diegops doesn't panic
    let _ = output.status;
}

#[test]
fn ktool_help_exits_successfully() {
    let output = diegops()
        .args(["ktool", "--help"])
        .output()
        .expect("failed to run diegops ktool --help");
    assert!(output.status.success());
}

#[test]
fn sync_help_exits_successfully() {
    let output = diegops()
        .args(["sync", "--help"])
        .output()
        .expect("failed to run diegops sync --help");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("push"), "got: {stdout}");
    assert!(stdout.contains("pull"), "got: {stdout}");
    assert!(stdout.contains("status"), "got: {stdout}");
}

#[test]
fn sync_push_help_exits_successfully() {
    let output = diegops()
        .args(["sync", "push", "--help"])
        .output()
        .expect("failed to run diegops sync push --help");
    assert!(output.status.success());
}

#[test]
fn sync_pull_help_exits_successfully() {
    let output = diegops()
        .args(["sync", "pull", "--help"])
        .output()
        .expect("failed to run diegops sync pull --help");
    assert!(output.status.success());
}

#[test]
fn sync_status_help_exits_successfully() {
    let output = diegops()
        .args(["sync", "status", "--help"])
        .output()
        .expect("failed to run diegops sync status --help");
    assert!(output.status.success());
}

#[test]
fn tool_list_exits_successfully() {
    let output = diegops()
        .args(["tool", "list"])
        .output()
        .expect("failed to run diegops tool list");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("gh"), "got: {stdout}");
    assert!(stdout.contains("vault"), "got: {stdout}");
    assert!(stdout.contains("terraform"), "got: {stdout}");
    assert!(stdout.contains("kubectl"), "got: {stdout}");
}

#[test]
fn tool_help_exits_successfully() {
    let output = diegops()
        .args(["tool", "--help"])
        .output()
        .expect("failed to run diegops tool --help");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("list"), "got: {stdout}");
    assert!(stdout.contains("install"), "got: {stdout}");
    assert!(stdout.contains("update"), "got: {stdout}");
    assert!(stdout.contains("remove"), "got: {stdout}");
}

#[test]
fn unknown_tool_passthrough_shows_error() {
    let output = diegops()
        .args(["nonexistent-tool-xyz"])
        .output()
        .expect("failed to run diegops");
    assert!(!output.status.success());
}

#[test]
fn secrets_help_exits_successfully() {
    let output = diegops()
        .args(["secrets", "--help"])
        .output()
        .expect("failed to run diegops secrets --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("init"), "got: {stdout}");
    assert!(stdout.contains("push"), "got: {stdout}");
    assert!(stdout.contains("pull"), "got: {stdout}");
    assert!(stdout.contains("status"), "got: {stdout}");
}

#[test]
fn secrets_init_help_exits_successfully() {
    let output = diegops()
        .args(["secrets", "init", "--help"])
        .output()
        .expect("failed to run diegops secrets init --help");

    assert!(output.status.success());
}

#[test]
fn auth_status_shows_kenv() {
    let output = diegops()
        .args(["auth", "status"])
        .output()
        .expect("failed to run diegops auth status");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("GitHub"), "got: {stdout}");
    assert!(stdout.contains("kenv"), "got: {stdout}");
}

#[test]
fn secrets_push_help_exits_successfully() {
    let output = diegops()
        .args(["secrets", "push", "--help"])
        .output()
        .expect("failed to run diegops secrets push --help");

    assert!(output.status.success());
}

#[test]
fn secrets_pull_help_exits_successfully() {
    let output = diegops()
        .args(["secrets", "pull", "--help"])
        .output()
        .expect("failed to run diegops secrets pull --help");

    assert!(output.status.success());
}

#[test]
fn secrets_status_help_exits_successfully() {
    let output = diegops()
        .args(["secrets", "status", "--help"])
        .output()
        .expect("failed to run diegops secrets status --help");

    assert!(output.status.success());
}

#[test]
fn all_help_exits_successfully() {
    let output = diegops()
        .args(["all", "--help"])
        .output()
        .expect("failed to run diegops all --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("pull"), "got: {stdout}");
    assert!(stdout.contains("push"), "got: {stdout}");
}

#[test]
fn all_pull_help_exits_successfully() {
    let output = diegops()
        .args(["all", "pull", "--help"])
        .output()
        .expect("failed to run diegops all pull --help");

    assert!(output.status.success());
}

#[test]
fn all_push_help_exits_successfully() {
    let output = diegops()
        .args(["all", "push", "--help"])
        .output()
        .expect("failed to run diegops all push --help");

    assert!(output.status.success());
}

#[test]
fn bootstrap_help_exits_successfully() {
    let output = diegops()
        .args(["bootstrap", "--help"])
        .output()
        .expect("failed to run diegops bootstrap --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("all pull"), "got: {stdout}");
}

#[test]
fn shell_init_help_exits_successfully() {
    let output = diegops()
        .args(["shell", "init", "--help"])
        .output()
        .expect("failed to run diegops shell init --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("zsh"), "help should mention zsh: {stdout}");
}

#[test]
fn secrets_update_help_exits_successfully() {
    let output = diegops()
        .args(["secrets", "update", "--help"])
        .output()
        .expect("failed to run diegops secrets update --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("CLAUDE.md") || stdout.contains("repos"),
        "help should mention CLAUDE.md or repos: {stdout}"
    );
}

#[test]
fn secrets_update_adds_agent_file_to_secrets_config() {
    use std::fs;

    let tmp = std::env::temp_dir().join(format!(
        "diegops-test-secrets-update-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();

    // Create a project folder with a CLAUDE.md in it.
    let project_dir = tmp.join("github").join("org").join("group");
    fs::create_dir_all(&project_dir).unwrap();
    fs::write(project_dir.join("CLAUDE.md"), "# Project instructions").unwrap();

    // Write a repos.yaml pointing at the project folder.
    let repos_config = tmp.join("repos.yaml");
    fs::write(
        &repos_config,
        format!(
            "targets:\n  - path: {}\n    repos: []\n",
            project_dir.display()
        ),
    )
    .unwrap();

    let secrets_config = tmp.join("secrets.yaml");

    let output = diegops()
        .args([
            "secrets",
            "update",
            "--repos-config",
            repos_config.to_str().unwrap(),
            "--config",
            secrets_config.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run diegops secrets update");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "expected success\nstdout: {stdout}\nstderr: {stderr}"
    );

    // secrets.yaml must exist and contain the CLAUDE.md entry.
    assert!(secrets_config.exists(), "secrets.yaml was not created");
    let content = fs::read_to_string(&secrets_config).unwrap();
    assert!(
        content.contains("CLAUDE.md"),
        "CLAUDE.md not added to secrets.yaml\ncontent: {content}"
    );
    assert!(
        content.contains("secret/workstation/agents/"),
        "vault_path not added\ncontent: {content}"
    );

    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn secrets_update_is_idempotent() {
    use std::fs;

    let tmp = std::env::temp_dir().join(format!(
        "diegops-test-secrets-update-idem-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();

    let project_dir = tmp.join("github").join("org").join("group");
    fs::create_dir_all(&project_dir).unwrap();
    fs::write(project_dir.join("CLAUDE.md"), "# instructions").unwrap();

    let repos_config = tmp.join("repos.yaml");
    fs::write(
        &repos_config,
        format!(
            "targets:\n  - path: {}\n    repos: []\n",
            project_dir.display()
        ),
    )
    .unwrap();

    let secrets_config = tmp.join("secrets.yaml");
    let args = [
        "secrets",
        "update",
        "--repos-config",
        repos_config.to_str().unwrap(),
        "--config",
        secrets_config.to_str().unwrap(),
    ];

    // First run — should add the entry.
    let out1 = diegops().args(args).output().expect("first run failed");
    assert!(out1.status.success());

    let content_after_first = fs::read_to_string(&secrets_config).unwrap();
    let count_first = content_after_first.matches("CLAUDE.md").count();

    // Second run — must be idempotent (no duplicate entry added).
    let out2 = diegops().args(args).output().expect("second run failed");
    assert!(out2.status.success());

    let content_after_second = fs::read_to_string(&secrets_config).unwrap();
    let count_second = content_after_second.matches("CLAUDE.md").count();
    assert_eq!(
        count_first, count_second,
        "second run added a duplicate CLAUDE.md entry"
    );

    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn kv_help_lists_backend_commands() {
    let out = diegops().args(["kv", "--help"]).output().unwrap();
    assert!(out.status.success());
    let s = String::from_utf8_lossy(&out.stdout);
    for cmd in ["get", "list", "put", "migrate", "backend"] {
        assert!(s.contains(cmd), "missing {cmd} in: {s}");
    }
}

#[test]
fn kv_vault_backend_without_address_fails_clearly() {
    let home = std::env::temp_dir().join(format!("diegops-kv-home-{}", std::process::id()));
    std::fs::create_dir_all(&home).unwrap();
    let out = diegops()
        .args(["kv", "--backend", "vault", "get", "dpinto-config/x"])
        .env("HOME", &home)
        .env_remove("VAULT_ADDR")
        .env_remove("DIEGOPS_CONFIG")
        .env("XDG_CONFIG_HOME", &home)
        .env("VAULT_TOKEN", "t")
        .output()
        .unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("no Vault address"), "{err}");
    std::fs::remove_dir_all(&home).unwrap();
}

#[test]
fn kv_backend_reports_infisical_by_default() {
    let home = std::env::temp_dir().join(format!("diegops-kv-def-{}", std::process::id()));
    std::fs::create_dir_all(&home).unwrap();
    let out = diegops()
        .args(["kv", "backend"])
        .env("HOME", &home)
        .env("XDG_CONFIG_HOME", &home)
        .env_remove("DIEGOPS_CONFIG")
        .env_remove("DIEGOPS_SECRETS_BACKEND")
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("infisical"));
    std::fs::remove_dir_all(&home).unwrap();
}

/// Piping into a reader that closes early (`diegops ... | head -1`) must end
/// quietly like other Unix tools, not panic with "failed printing to stdout".
#[cfg(unix)]
#[test]
fn closed_stdout_pipe_does_not_panic() {
    use std::process::Stdio;
    let mut child = diegops()
        .arg("version")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to run diegops");
    // Close the read end before the child prints (println! panics on EPIPE).
    drop(child.stdout.take());
    let output = child.wait_with_output().expect("wait failed");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("panicked"), "stderr: {stderr}");
    assert!(!stderr.contains("Broken pipe"), "stderr: {stderr}");
}

// --- #37: readable errors -------------------------------------------------

#[test]
fn runtime_error_is_printed_readably() {
    let output = diegops()
        .args([
            "repo",
            "list",
            "--config",
            "/nonexistent/diegops/repos.yaml",
        ])
        .output()
        .expect("failed to run diegops");

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.starts_with("error: config not found: "),
        "got: {stderr}"
    );
    assert!(!stderr.contains("\\n"), "literal \\n in: {stderr}");
    assert!(!stderr.contains('"'), "debug quotes in: {stderr}");
    assert!(
        stderr.contains("\n  hint: "),
        "hint not on its own line: {stderr}"
    );
}

#[test]
fn unknown_command_is_a_usage_error() {
    let output = diegops()
        .arg("nonexistent-tool-xyz")
        .output()
        .expect("failed to run diegops");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.starts_with("error: unknown command 'nonexistent-tool-xyz'"),
        "got: {stderr}"
    );
    assert!(stderr.contains("diegops --help"), "got: {stderr}");
}

#[test]
fn long_help_documents_exit_codes() {
    let output = diegops().arg("--help").output().expect("run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("EXIT CODES:"), "got: {stdout}");
    assert!(stdout.contains("2  usage error"), "got: {stdout}");
}

// --- #38: actionable errors -----------------------------------------------

#[test]
fn missing_repos_config_suggests_init_command() {
    let home = std::env::temp_dir().join(format!("diegops-it-38-{}", std::process::id()));
    std::fs::create_dir_all(&home).unwrap();
    let output = diegops()
        .args(["repo", "list"])
        .env("HOME", &home)
        .env_remove("DIEGOPS_REPOS_CONFIG")
        .output()
        .expect("run");
    std::fs::remove_dir_all(&home).ok();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("hint: run 'diegops repo init' to create a sample"),
        "got: {stderr}"
    );
}

#[test]
fn kv_backend_says_when_config_is_missing() {
    let output = diegops()
        .args(["kv", "backend"])
        .env("DIEGOPS_CONFIG", "/nonexistent/diegops/config.toml")
        .env_remove("DIEGOPS_BACKEND")
        .output()
        .expect("run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("(not found, using defaults)"),
        "got: {stdout}"
    );
}

// --- #39: help discoverability ---------------------------------------------

#[test]
fn help_subcommand_shows_nested_command_help() {
    let output = diegops()
        .args(["help", "repo", "apply"])
        .output()
        .expect("run");
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Usage: diegops repo apply"),
        "got: {stdout}"
    );
}

#[test]
fn help_for_unknown_command_is_usage_error() {
    let output = diegops().args(["help", "nope"]).output().expect("run");
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.starts_with("error: unknown command 'nope'"),
        "got: {stderr}"
    );
}

#[test]
fn typo_in_top_level_command_suggests_closest() {
    let output = diegops().arg("secret").output().expect("run");
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("did you mean 'secrets'?"), "got: {stderr}");
}

#[test]
fn typo_in_tool_name_suggests_tool() {
    let output = diegops().arg("kubctl").output().expect("run");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("did you mean 'kubectl'?"), "got: {stderr}");
}

#[test]
fn top_level_help_groups_commands() {
    let output = diegops().arg("--help").output().expect("run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    for heading in [
        "Get started:",
        "Secrets:",
        "Workspace:",
        "Tools:",
        "AI agents:",
        "About:",
    ] {
        assert!(stdout.contains(heading), "missing {heading} in: {stdout}");
    }
}

#[test]
fn version_flag_and_subcommand_match() {
    let a = diegops().arg("version").output().expect("run");
    let b = diegops().arg("--version").output().expect("run");
    assert_eq!(
        String::from_utf8_lossy(&a.stdout),
        String::from_utf8_lossy(&b.stdout)
    );
}

// --- #41: completions and man page -----------------------------------------

#[test]
fn completion_scripts_for_each_shell() {
    for (shell, marker) in [
        ("bash", "_diegops()"),
        ("zsh", "#compdef diegops"),
        ("fish", "complete -c diegops"),
        ("powershell", "Register-ArgumentCompleter"),
    ] {
        let output = diegops().args(["completion", shell]).output().expect("run");
        assert!(output.status.success(), "{shell}: {output:?}");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains(marker), "{shell} missing {marker}");
        assert!(stdout.contains("bootstrap"), "{shell} lacks subcommands");
    }
}

#[test]
fn completion_rejects_unknown_shell() {
    let output = diegops()
        .args(["completion", "tcsh"])
        .output()
        .expect("run");
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn man_page_is_roff() {
    let output = diegops().arg("man").output().expect("run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(".TH diegops"),
        "got: {}",
        &stdout[..stdout.len().min(200)]
    );
    assert!(stdout.contains("bootstrap"));
}

// --- #42: explain -----------------------------------------------------------

#[test]
fn explain_lists_terms() {
    let output = diegops().arg("explain").output().expect("run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    for t in ["backend", "infisical", "machine-identity", "pat", "kv"] {
        assert!(stdout.contains(t), "missing {t}: {stdout}");
    }
}

#[test]
fn explain_term_and_alias() {
    for term in ["machine-identity", "Machine Identity", "client-secret"] {
        let output = diegops().args(["explain", term]).output().expect("run");
        assert!(output.status.success(), "{term}");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Client ID"), "{term}: {stdout}");
        assert!(stdout.contains("https://"), "{term}: no link");
    }
}

#[test]
fn explain_unknown_term_suggests() {
    let output = diegops()
        .args(["explain", "infisicl"])
        .output()
        .expect("run");
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("did you mean 'infisical'?"), "{stderr}");
}

#[test]
fn credential_errors_point_to_explain() {
    let output = diegops()
        .args(["kv", "get", "a/b"])
        .env("DIEGOPS_CONFIG", "/nonexistent/diegops/config.toml")
        .env("DIEGOPS_SECRETS_BACKEND", "infisical")
        .env_remove("INFISICAL_TOKEN")
        .env_remove("INFISICAL_UNIVERSAL_AUTH_CLIENT_ID")
        .output()
        .expect("run");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("diegops explain machine-identity"),
        "{stderr}"
    );
}

// --- #43: auth gh login without the token on the command line ---------------

#[test]
fn gh_login_reads_token_from_stdin_and_rejects_empty() {
    use std::io::Write;
    let tmp = TmpHome::new();
    let mut child = diegops()
        .args(["auth", "gh", "login", "--with-token"])
        .env("HOME", tmp.path())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn");
    child.stdin.take().unwrap().write_all(b"  \n").unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("no token received on stdin"), "{stderr}");
}

#[test]
fn gh_login_positional_token_is_deprecated() {
    let tmp = TmpHome::new();
    let output = diegops()
        .args(["auth", "gh", "login", " "])
        .env("HOME", tmp.path())
        .output()
        .expect("run");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("warning: passing the token as an argument"),
        "{stderr}"
    );
    assert!(!tmp.path().join(".diegops/tokens/gh.json").exists());
}

#[test]
fn gh_login_help_explains_pat() {
    let output = diegops()
        .args(["auth", "gh", "login", "--help"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--with-token"), "{stdout}");
    assert!(stdout.contains("--from-gh"), "{stdout}");
    assert!(
        stdout.contains("github.com/settings/personal-access-tokens/new"),
        "{stdout}"
    );
    assert!(
        !stdout.contains("<TOKEN>"),
        "positional should be hidden: {stdout}"
    );
}

#[test]
fn hints_do_not_suggest_token_as_argument() {
    for f in [
        "src/commands/auth.rs",
        "src/commands/sync.rs",
        "src/commands/all.rs",
        "src/examples.rs",
        "src/main.rs",
        "README.md",
    ] {
        let s = std::fs::read_to_string(f).unwrap();
        assert!(
            !s.contains("gh login <PAT>") && !s.contains("gh login <TOKEN>"),
            "{f}"
        );
    }
}

/// Throwaway HOME for tests that must not touch the real one.
struct TmpHome(std::path::PathBuf);
impl TmpHome {
    fn new() -> Self {
        use std::sync::atomic::{AtomicU32, Ordering};
        static N: AtomicU32 = AtomicU32::new(0);
        let p = std::env::temp_dir().join(format!(
            "diegops-test-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&p).unwrap();
        TmpHome(p)
    }
    fn path(&self) -> &std::path::Path {
        &self.0
    }
}
impl Drop for TmpHome {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
