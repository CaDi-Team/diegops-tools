//! The `diegops bootstrap` command — full workstation setup in one shot.
//!
//! Delegates the actual sync sequence to `all::pull()` and wraps it with the
//! hero banner and a final "ready" message.

/// Bootstraps a fresh workstation: verifies auth, pulls config, clones repos,
/// injects secrets, and restores workstation files.
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let results = super::all::pull()?;

    eprintln!();
    super::cadi::run();
    eprintln!();

    let failures = super::all::print_summary(&results);

    eprintln!();
    if failures == 0 {
        eprintln!("Workstation ready.");
        Ok(())
    } else {
        eprintln!("Workstation ready with {failures} error(s).");
        Err(format!("{failures} bootstrap step(s) failed").into())
    }
}
