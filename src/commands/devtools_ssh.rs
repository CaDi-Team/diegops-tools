//! SSH key management.

/// Lists local SSH keys and GitHub registered keys.
pub fn list() -> Result<(), Box<dyn std::error::Error>> {
    todo!("devtools ssh list")
}

/// Prints ~/.ssh/config contents.
pub fn config() -> Result<(), Box<dyn std::error::Error>> {
    todo!("devtools ssh config")
}

/// Creates a new SSH key.
pub fn create(
    _name: Option<&str>,
    _key_type: &str,
    _email: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    todo!("devtools ssh create")
}
