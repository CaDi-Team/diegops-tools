//! Command implementations.
//!
//! Each non-trivial command lives in its own submodule and exposes a single
//! `pub fn run(...) -> Result<(), Box<dyn std::error::Error>>` entry point.

pub mod update;
