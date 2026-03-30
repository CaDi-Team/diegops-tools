//! Command implementations.
//!
//! Each non-trivial command lives in its own submodule and exposes a single
//! `pub fn run(...) -> Result<(), Box<dyn std::error::Error>>` entry point.

pub mod auth;
pub mod cadi;
pub mod common;
pub mod devtools;
pub mod devtools_git;
pub mod devtools_gpg;
pub mod devtools_ssh;
pub mod ktool;
pub mod repo;
pub mod secrets;
pub mod sync;
pub mod tool;
pub mod update;
pub mod vault;
