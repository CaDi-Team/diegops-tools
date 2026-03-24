//! GPG key management and git commit signing setup.

/// Generates GPG identity config.
pub fn init(_name: Option<&str>, _email: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    todo!("devtools gpg init")
}

/// Generates GPG key, uploads to GitHub, configures git signing.
pub fn set() -> Result<(), Box<dyn std::error::Error>> {
    todo!("devtools gpg set")
}

/// Restarts gpg-agent.
pub fn restart() -> Result<(), Box<dyn std::error::Error>> {
    todo!("devtools gpg restart")
}
