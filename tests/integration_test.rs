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
fn vault_list_diff_help_exits_successfully() {
    let output = diegops()
        .args(["vault", "list-diff", "--help"])
        .output()
        .expect("failed to run diegops vault list-diff --help");

    assert!(output.status.success());
}
