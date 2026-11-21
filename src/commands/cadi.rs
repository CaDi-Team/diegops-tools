//! The `diegops cadi` hero screen — retro ASCII art branding.

const ART: &[&str] = &[
    "",
    r"     ___  _            ___  ___",
    r"    /   \(_) ___  __ _/___\/ _ \___",
    r"   / /\ /| |/ _ \/ _` //  // /_)/ __|",
    r"  / /_// | |  __/ (_| / \_// ___/\__ \",
    r" /___,'  |_|\___|\__, \___/\/    |___/",
    r"                 |___/",
    "^DiegOps",
    "",
    "   \"When more than one Diego is needed\"",
    "",
];

/// The hero screen, with the right border computed so it always lines up.
pub fn banner() -> String {
    let version = format!(
        "   v{} · Made by CaDi Labs with love <3",
        env!("CARGO_PKG_VERSION")
    );
    let mut lines: Vec<String> = ART.iter().map(|s| s.to_string()).collect();
    lines.push(version);
    lines.push(String::new());
    let widest = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
    // Lines starting with '^' are centred.
    for l in lines.iter_mut() {
        if let Some(t) = l.strip_prefix('^') {
            *l = format!("{}{t}", " ".repeat((widest - t.len()) / 2));
        }
    }
    let inner = widest + 4;
    let mut out = format!(" ╔{}╗\n", "═".repeat(inner));
    for l in &lines {
        let pad = inner - 1 - l.chars().count();
        out.push_str(&format!(" ║ {l}{}║\n", " ".repeat(pad)));
    }
    out.push_str(&format!(" ╚{}╝", "═".repeat(inner)));
    out
}

/// Prints the DiegOps hero screen to stdout.
pub fn run() {
    println!("{}", banner());
}

#[cfg(test)]
mod tests {
    #[test]
    fn banner_borders_line_up() {
        let b = super::banner();
        let widths: Vec<usize> = b.lines().map(|l| l.chars().count()).collect();
        assert!(widths.iter().all(|w| *w == widths[0]), "{widths:?}\n{b}");
    }
}
