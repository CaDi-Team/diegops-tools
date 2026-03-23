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
