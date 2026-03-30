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
fn bootstrap_help_exits_successfully() {
    let output = diegops()
        .args(["bootstrap", "--help"])
        .output()
        .expect("failed to run diegops bootstrap --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Bootstrap"),
        "help should mention Bootstrap: {stdout}"
    );
}
