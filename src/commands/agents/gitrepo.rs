//! Minimal git wrapper for the agent backup repo (shells out to `git`, which
//! already holds the user's credentials/SSH setup).

use std::path::PathBuf;
use std::process::Command;

type BoxError = Box<dyn std::error::Error>;

/// The local clone of the backup repo.
pub struct Repo {
    /// Clone path.
    pub path: PathBuf,
    /// Branch override (default: current branch).
    pub branch: Option<String>,
    /// Clone URL for a missing clone.
    pub url: Option<String>,
}

impl Repo {
    fn git(&self, args: &[&str]) -> Result<String, BoxError> {
        let out = Command::new("git")
            .arg("-C")
            .arg(&self.path)
            .args(args)
            .output()
            .map_err(|e| format!("could not run git: {e}"))?;
        if !out.status.success() {
            return Err(format!(
                "git {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            )
            .into());
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    /// Clones the repo when the local path is missing.
    pub fn ensure(&self) -> Result<(), BoxError> {
        if self.path.join(".git").exists() {
            return Ok(());
        }
        let url = self.url.as_deref().ok_or_else(|| {
            format!(
                "{} is not a git clone and agents.yaml has no repo.url to clone from",
                self.path.display()
            )
        })?;
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let st = Command::new("git")
            .args(["clone", "--quiet", url])
            .arg(&self.path)
            .status()?;
        if !st.success() {
            return Err(format!("git clone {url} failed").into());
        }
        Ok(())
    }

    fn branch(&self) -> Result<String, BoxError> {
        match &self.branch {
            Some(b) => Ok(b.clone()),
            None => self.git(&["rev-parse", "--abbrev-ref", "HEAD"]),
        }
    }

    /// Fast-forwards from origin (no-op without a remote).
    pub fn pull(&self) -> Result<(), BoxError> {
        if self.git(&["remote"])?.is_empty() {
            return Ok(());
        }
        let b = self.branch()?;
        self.git(&["pull", "--ff-only", "--quiet", "origin", &b])?;
        Ok(())
    }

    /// Short HEAD hash.
    pub fn head(&self) -> Result<String, BoxError> {
        self.git(&["rev-parse", "--short", "HEAD"])
    }

    /// Stages `paths`, commits if anything changed, pushes; returns HEAD.
    pub fn commit_push(&self, paths: &[&str], msg: &str) -> Result<String, BoxError> {
        let mut add = vec!["add", "-A", "--"];
        add.extend_from_slice(paths);
        self.git(&add)?;
        let mut diff = vec!["diff", "--cached", "--quiet", "--"];
        diff.extend_from_slice(paths);
        let changed = Command::new("git")
            .arg("-C")
            .arg(&self.path)
            .args(&diff)
            .status()?
            .code()
            == Some(1);
        if changed {
            let mut commit = vec!["commit", "--quiet", "-m", msg, "--"];
            commit.extend_from_slice(paths);
            self.git(&commit)?;
            if !self.git(&["remote"])?.is_empty() {
                let b = self.branch()?;
                self.git(&["push", "--quiet", "origin", &format!("HEAD:{b}")])?;
            }
        } else {
            eprintln!("  no changes to commit");
        }
        self.head()
    }
}
