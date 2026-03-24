//! The `diegops cadi` hero screen — retro ASCII art branding.

/// Prints the DiegOps hero screen to stdout.
pub fn run() {
    println!(
        r#" ╔═══════════════════════════════════════════════════════╗
 ║                                                       ║
 ║      ___  _            ___  ___                       ║
 ║     /   \(_) ___  __ _/___\/ _ \___                   ║
 ║    / /\ /| |/ _ \/ _` //  // /_)/ __|                 ║
 ║   / /_// | |  __/ (_| / \_// ___/\__ \                 ║
 ║  /___,'  |_|\___|\__, \___/\/    |___/                 ║
 ║                  |___/                                 ║
 ║                        DiegOps                         ║
 ║                                                       ║
 ║    "When more than one Diego is needed"                ║
 ║                                                       ║
 ║    v{version} · Made by CaDi Labs with love <3         ║
 ║                                                       ║
 ╚═══════════════════════════════════════════════════════╝"#,
        version = env!("CARGO_PKG_VERSION")
    );
}
