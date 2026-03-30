//! Sync command — back up and restore `~/.diegops/` config files to a private
//! GitHub repository via the GitHub REST API.
//!
//! Sync scope: everything in `~/.diegops/` **except** `tokens/` and `bin/`.

use base64::Engine;
use sha1::{Digest, Sha1};
use std::fs;
use std::path::Path;

/// Sync sub-commands.
#[derive(clap::Subcommand)]
pub enum SyncCommand {
    /// Upload local config files to GitHub (creates repo on first use)
    #[command(long_about = "Upload ~/.diegops/ config files to GitHub.\n\n\
        On first push, creates a private repo: diegops-{username}-memory\n\
        Excludes tokens/ and bin/ directories.\n\
        Skips unchanged files (content-aware).")]
    Push,
    /// Download config files from GitHub to local ~/.diegops/
    #[command(long_about = "Download config files from GitHub to ~/.diegops/\n\n\
        Overwrites local files with cloud versions.\n\
        Skips unchanged files. Creates directories as needed.\n\
        Requires: run 'diegops sync push' first to create the repo.")]
    Pull,
    /// Show diff between local and cloud config files
    #[command(long_about = "Show diff between local and cloud config files.\n\n\
        Legend: = in sync, ~ differs, + local only, - cloud only")]
    Status,
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const GITHUB_API: &str = "https://api.github.com";

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Builds the `User-Agent` header value for GitHub API requests.
fn user_agent() -> String {
    format!("diegops/{}", env!("CARGO_PKG_VERSION"))
}

/// Loads the stored GitHub token, returning a user-friendly error if missing.
fn require_token() -> Result<String, Box<dyn std::error::Error>> {
    super::auth::load_gh_token()?
        .ok_or_else(|| "not authenticated. Run 'diegops auth gh login <PAT>' first".into())
}

/// Fetches the authenticated GitHub username.
fn get_username(token: &str) -> Result<String, Box<dyn std::error::Error>> {
    let resp = ureq::get(&format!("{GITHUB_API}/user"))
        .set("User-Agent", &user_agent())
        .set("Authorization", &format!("Bearer {token}"))
        .set("Accept", "application/vnd.github.v3+json")
        .call();

    match resp {
        Ok(r) => {
            let json: serde_json::Value = r.into_json()?;
            json["login"]
                .as_str()
                .map(String::from)
                .ok_or_else(|| "GitHub API response missing 'login'".into())
        }
        Err(ureq::Error::Status(401 | 403, _)) => {
            Err("GitHub token is invalid or expired. Run 'diegops auth gh login <PAT>'".into())
        }
        Err(e) => Err(format!("GitHub API error: {e}").into()),
    }
}

/// Returns the sync repository name for the given user.
fn repo_name(username: &str) -> String {
    format!("diegops-{username}-memory")
}

/// Checks whether the sync repository already exists on GitHub.
fn repo_exists(token: &str, owner: &str, repo: &str) -> Result<bool, Box<dyn std::error::Error>> {
    let url = format!("{GITHUB_API}/repos/{owner}/{repo}");
    let resp = ureq::get(&url)
        .set("User-Agent", &user_agent())
        .set("Authorization", &format!("Bearer {token}"))
        .set("Accept", "application/vnd.github.v3+json")
        .call();

    match resp {
        Ok(_) => Ok(true),
        Err(ureq::Error::Status(404, _)) => Ok(false),
        Err(ureq::Error::Status(401 | 403, _)) => {
            Err("GitHub token is invalid or expired. Run 'diegops auth gh login <PAT>'".into())
        }
        Err(e) => Err(format!("GitHub API error: {e}").into()),
    }
}

/// Creates the sync repository on GitHub (private, no auto-init).
fn create_repo(token: &str, name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let body = serde_json::json!({
        "name": name,
        "private": true,
        "auto_init": false,
    });

    let resp = ureq::post(&format!("{GITHUB_API}/user/repos"))
        .set("User-Agent", &user_agent())
        .set("Authorization", &format!("Bearer {token}"))
        .set("Accept", "application/vnd.github.v3+json")
        .send_json(body);

    match resp {
        Ok(_) => Ok(()),
        Err(ureq::Error::Status(422, _)) => {
            // 422 usually means the repo already exists — treat as success (idempotent)
            Ok(())
        }
        Err(ureq::Error::Status(401 | 403, _)) => {
            Err("GitHub token is invalid or lacks repo creation scope".into())
        }
        Err(e) => Err(format!("failed to create repository: {e}").into()),
    }
}

/// Ensures the sync repository exists, creating it if necessary. Returns `(owner, repo)`.
fn ensure_repo(
    token: &str,
    username: &str,
) -> Result<(String, String), Box<dyn std::error::Error>> {
    let repo = repo_name(username);
    if !repo_exists(token, username, &repo)? {
        eprintln!("Creating repository {username}/{repo} ...");
        create_repo(token, &repo)?;
    }
    Ok((username.to_owned(), repo))
}

/// A local file with its relative path and content.
type LocalFile = (String, Vec<u8>);

/// Scans `~/.diegops/` recursively, skipping `tokens/` and `bin/` at the top level.
/// Returns a list of `(relative_path, content)` pairs.
fn scan_local_files(diegops_dir: &Path) -> Result<Vec<LocalFile>, Box<dyn std::error::Error>> {
    let mut files = Vec::new();
    if diegops_dir.is_dir() {
        scan_dir(diegops_dir, diegops_dir, &mut files)?;
    }
    Ok(files)
}

/// Recursive directory walker for [`scan_local_files`].
fn scan_dir(
    base: &Path,
    dir: &Path,
    files: &mut Vec<LocalFile>,
) -> Result<(), Box<dyn std::error::Error>> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let rel = path
            .strip_prefix(base)?
            .to_string_lossy()
            .replace('\\', "/");

        // Skip excluded top-level directories
        let top_dir = rel.split('/').next().unwrap_or("");
        if top_dir == "tokens" || top_dir == "bin" {
            continue;
        }

        if path.is_dir() {
            scan_dir(base, &path, files)?;
        } else {
            let content = fs::read(&path)?;
            files.push((rel.to_string(), content));
        }
    }
    Ok(())
}

/// Computes the GitHub blob SHA for a piece of content.
///
/// GitHub uses `SHA1("blob {size}\0{content}")`.
fn github_blob_sha(content: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(format!("blob {}\0", content.len()).as_bytes());
    hasher.update(content);
    hasher.finalize().iter().fold(String::new(), |mut acc, b| {
        use std::fmt::Write;
        let _ = write!(acc, "{b:02x}");
        acc
    })
}

/// Metadata for a file stored on GitHub.
struct CloudFile {
    /// SHA of the blob (used for update detection and PUT requests).
    sha: String,
    /// Base64-encoded content (only present when fetched with media type).
    content: Option<String>,
}

/// Fetches a single file's metadata (SHA and optionally content) from GitHub.
/// Returns `None` if the file does not exist (404).
fn get_cloud_file(
    token: &str,
    owner: &str,
    repo: &str,
    path: &str,
) -> Result<Option<CloudFile>, Box<dyn std::error::Error>> {
    let url = format!("{GITHUB_API}/repos/{owner}/{repo}/contents/{path}");
    let resp = ureq::get(&url)
        .set("User-Agent", &user_agent())
        .set("Authorization", &format!("Bearer {token}"))
        .set("Accept", "application/vnd.github.v3+json")
        .call();

    match resp {
        Ok(r) => {
            let json: serde_json::Value = r.into_json()?;
            let sha = json["sha"]
                .as_str()
                .ok_or("missing sha in GitHub contents response")?
                .to_owned();
            let content = json["content"].as_str().map(|s| s.to_owned());
            Ok(Some(CloudFile { sha, content }))
        }
        Err(ureq::Error::Status(404, _)) => Ok(None),
        Err(ureq::Error::Status(401 | 403, _)) => {
            Err("GitHub token is invalid or expired. Run 'diegops auth gh login <PAT>'".into())
        }
        Err(e) => Err(format!("GitHub API error fetching {path}: {e}").into()),
    }
}

/// Creates or updates a file on GitHub via the Contents API.
fn put_cloud_file(
    token: &str,
    owner: &str,
    repo: &str,
    path: &str,
    content: &[u8],
    existing_sha: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let b64 = base64::engine::general_purpose::STANDARD.encode(content);
    let mut body = serde_json::json!({
        "message": format!("sync: update {path}"),
        "content": b64,
    });
    if let Some(sha) = existing_sha {
        body["sha"] = serde_json::Value::String(sha.to_owned());
    }

    let url = format!("{GITHUB_API}/repos/{owner}/{repo}/contents/{path}");
    let resp = ureq::put(&url)
        .set("User-Agent", &user_agent())
        .set("Authorization", &format!("Bearer {token}"))
        .set("Accept", "application/vnd.github.v3+json")
        .send_json(body);

    match resp {
        Ok(_) => Ok(()),
        Err(ureq::Error::Status(401 | 403, _)) => {
            Err("GitHub token is invalid or lacks write access".into())
        }
        Err(e) => Err(format!("failed to upload {path}: {e}").into()),
    }
}

/// Lists all files in a GitHub repo directory recursively.
/// Returns `(path, sha)` pairs for every file.
fn list_cloud_files(
    token: &str,
    owner: &str,
    repo: &str,
    dir_path: &str,
) -> Result<Vec<(String, String)>, Box<dyn std::error::Error>> {
    let url = if dir_path.is_empty() {
        format!("{GITHUB_API}/repos/{owner}/{repo}/contents/")
    } else {
        format!("{GITHUB_API}/repos/{owner}/{repo}/contents/{dir_path}")
    };

    let resp = ureq::get(&url)
        .set("User-Agent", &user_agent())
        .set("Authorization", &format!("Bearer {token}"))
        .set("Accept", "application/vnd.github.v3+json")
        .call();

    match resp {
        Ok(r) => {
            let json: serde_json::Value = r.into_json()?;
            let mut files = Vec::new();

            if let Some(arr) = json.as_array() {
                for item in arr {
                    let item_type = item["type"].as_str().unwrap_or("");
                    let item_path = item["path"].as_str().unwrap_or("").to_owned();
                    let item_sha = item["sha"].as_str().unwrap_or("").to_owned();

                    if item_type == "file" {
                        files.push((item_path, item_sha));
                    } else if item_type == "dir" {
                        // Recurse into subdirectory
                        let sub = list_cloud_files(token, owner, repo, &item_path)?;
                        files.extend(sub);
                    }
                }
            }

            Ok(files)
        }
        // Empty repo returns 404 for contents/
        Err(ureq::Error::Status(404, _)) => Ok(Vec::new()),
        Err(ureq::Error::Status(401 | 403, _)) => {
            Err("GitHub token is invalid or expired. Run 'diegops auth gh login <PAT>'".into())
        }
        Err(e) => Err(format!("GitHub API error listing {dir_path}: {e}").into()),
    }
}

/// Decodes the base64 content field returned by GitHub (which may contain newlines).
fn decode_github_content(encoded: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // GitHub's base64 includes line breaks — strip them before decoding
    let clean: String = encoded.chars().filter(|c| !c.is_whitespace()).collect();
    base64::engine::general_purpose::STANDARD
        .decode(&clean)
        .map_err(|e| format!("base64 decode error: {e}").into())
}

// ---------------------------------------------------------------------------
// Command handlers
// ---------------------------------------------------------------------------

/// Uploads local `~/.diegops/` config files to GitHub.
pub fn push() -> Result<(), Box<dyn std::error::Error>> {
    let token = require_token()?;
    let username = get_username(&token)?;
    let (owner, repo) = ensure_repo(&token, &username)?;

    let diegops_dir = super::common::diegops_dir()?;
    let files = scan_local_files(&diegops_dir)?;

    if files.is_empty() {
        println!("No config files found in {}", diegops_dir.display());
        return Ok(());
    }

    let mut uploaded = 0u32;
    let mut skipped = 0u32;
    let mut failed = 0u32;

    for (rel_path, content) in &files {
        let local_sha = github_blob_sha(content);

        // Check if cloud already has this exact content
        match get_cloud_file(&token, &owner, &repo, rel_path) {
            Ok(Some(cloud)) if cloud.sha == local_sha => {
                eprintln!("  SKIP  {rel_path} (unchanged)");
                skipped += 1;
            }
            Ok(existing) => {
                let sha = existing.as_ref().map(|f| f.sha.as_str());
                match put_cloud_file(&token, &owner, &repo, rel_path, content, sha) {
                    Ok(()) => {
                        eprintln!("  PUSH  {rel_path}");
                        uploaded += 1;
                    }
                    Err(e) => {
                        eprintln!("  FAIL  {rel_path}: {e}");
                        failed += 1;
                    }
                }
            }
            Err(e) => {
                eprintln!("  FAIL  {rel_path}: {e}");
                failed += 1;
            }
        }
    }

    println!("Push complete: {uploaded} uploaded, {skipped} unchanged, {failed} failed");
    Ok(())
}

/// Downloads config files from GitHub to `~/.diegops/`.
pub fn pull() -> Result<(), Box<dyn std::error::Error>> {
    let token = require_token()?;
    let username = get_username(&token)?;
    let repo = repo_name(&username);

    if !repo_exists(&token, &username, &repo)? {
        return Err(format!(
            "sync repository {username}/{repo} does not exist. Run 'diegops sync push' first"
        )
        .into());
    }

    let cloud_files = list_cloud_files(&token, &username, &repo, "")?;

    if cloud_files.is_empty() {
        println!("No files in cloud repository");
        return Ok(());
    }

    let diegops_dir = super::common::diegops_dir()?;
    let mut written = 0u32;
    let mut skipped = 0u32;
    let mut failed = 0u32;

    for (path, _sha) in &cloud_files {
        // Fetch full content for each file
        match get_cloud_file(&token, &username, &repo, path) {
            Ok(Some(cloud)) => {
                let content_b64 = match &cloud.content {
                    Some(c) => c.clone(),
                    None => {
                        eprintln!("  FAIL  {path}: no content returned");
                        failed += 1;
                        continue;
                    }
                };

                let content = match decode_github_content(&content_b64) {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("  FAIL  {path}: {e}");
                        failed += 1;
                        continue;
                    }
                };

                let local_path = diegops_dir.join(path);

                // Skip if local file already matches
                if local_path.exists() {
                    if let Ok(local_content) = fs::read(&local_path) {
                        if local_content == content {
                            eprintln!("  SKIP  {path} (unchanged)");
                            skipped += 1;
                            continue;
                        }
                    }
                }

                // Ensure parent directory exists
                if let Some(parent) = local_path.parent() {
                    fs::create_dir_all(parent)?;
                }

                match fs::write(&local_path, &content) {
                    Ok(()) => {
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::PermissionsExt;
                            let _ =
                                fs::set_permissions(&local_path, fs::Permissions::from_mode(0o600));
                        }
                        eprintln!("  PULL  {path}");
                        written += 1;
                    }
                    Err(e) => {
                        eprintln!("  FAIL  {path}: {e}");
                        failed += 1;
                    }
                }
            }
            Ok(None) => {
                eprintln!("  FAIL  {path}: file disappeared from cloud");
                failed += 1;
            }
            Err(e) => {
                eprintln!("  FAIL  {path}: {e}");
                failed += 1;
            }
        }
    }

    println!("Pull complete: {written} written, {skipped} unchanged, {failed} failed");
    Ok(())
}

/// Shows the diff between local and cloud config files.
pub fn status() -> Result<(), Box<dyn std::error::Error>> {
    let token = require_token()?;
    let username = get_username(&token)?;
    let repo = repo_name(&username);

    let has_repo = repo_exists(&token, &username, &repo)?;

    let diegops_dir = super::common::diegops_dir()?;
    let local_files = scan_local_files(&diegops_dir)?;

    // Build a map of local file path -> blob SHA
    let local_map: std::collections::HashMap<String, String> = local_files
        .iter()
        .map(|(path, content)| (path.clone(), github_blob_sha(content)))
        .collect();

    // Build a map of cloud file path -> blob SHA
    let cloud_map: std::collections::HashMap<String, String> = if has_repo {
        list_cloud_files(&token, &username, &repo, "")?
            .into_iter()
            .collect()
    } else {
        std::collections::HashMap::new()
    };

    // Collect all known paths
    let mut all_paths: Vec<String> = local_map.keys().cloned().collect();
    for k in cloud_map.keys() {
        if !local_map.contains_key(k) {
            all_paths.push(k.clone());
        }
    }
    all_paths.sort();

    if all_paths.is_empty() {
        println!("No config files found locally or in the cloud");
        return Ok(());
    }

    let mut in_sync = 0u32;
    let mut differs = 0u32;
    let mut local_only = 0u32;
    let mut cloud_only = 0u32;

    for path in &all_paths {
        let local_sha = local_map.get(path);
        let cloud_sha = cloud_map.get(path);

        match (local_sha, cloud_sha) {
            (Some(l), Some(c)) if l == c => {
                println!("  =  {path}");
                in_sync += 1;
            }
            (Some(_), Some(_)) => {
                println!("  ~  {path}");
                differs += 1;
            }
            (Some(_), None) => {
                println!("  +  {path}  (local only)");
                local_only += 1;
            }
            (None, Some(_)) => {
                println!("  -  {path}  (cloud only)");
                cloud_only += 1;
            }
            (None, None) => {
                // Should not happen
            }
        }
    }

    println!();
    println!(
        "Summary: {in_sync} in sync, {differs} differ, {local_only} local only, {cloud_only} cloud only"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_blob_sha_matches_known_value() {
        // "hello\n" -> blob SHA should be ce013625030ba8dba906f756967f9e9ca394464a
        let content = b"hello\n";
        let sha = github_blob_sha(content);
        assert_eq!(sha, "ce013625030ba8dba906f756967f9e9ca394464a");
    }

    #[test]
    fn github_blob_sha_empty() {
        let sha = github_blob_sha(b"");
        assert_eq!(sha, "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391");
    }

    #[test]
    fn decode_github_content_strips_whitespace() {
        // GitHub returns base64 with line breaks
        let encoded = "aGVsbG8g\nd29ybGQ=\n";
        let decoded = decode_github_content(encoded).unwrap();
        assert_eq!(decoded, b"hello world");
    }

    #[test]
    fn repo_name_format() {
        assert_eq!(repo_name("octocat"), "diegops-octocat-memory");
    }

    #[test]
    fn scan_local_files_skips_excluded_dirs() {
        let dir =
            std::env::temp_dir().join(format!("diegops-test-sync-scan-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);

        // Create structure
        fs::create_dir_all(dir.join("tokens")).unwrap();
        fs::create_dir_all(dir.join("bin")).unwrap();
        fs::create_dir_all(dir.join("subdir")).unwrap();
        fs::write(dir.join("tokens").join("gh.json"), "secret").unwrap();
        fs::write(dir.join("bin").join("tool"), "binary").unwrap();
        fs::write(dir.join("repos.yaml"), "repos").unwrap();
        fs::write(dir.join("subdir").join("nested.yaml"), "nested").unwrap();

        let files = scan_local_files(&dir).unwrap();
        let paths: Vec<&str> = files.iter().map(|(p, _)| p.as_str()).collect();

        assert!(paths.contains(&"repos.yaml"));
        assert!(paths.contains(&"subdir/nested.yaml"));
        assert!(!paths.iter().any(|p| p.starts_with("tokens")));
        assert!(!paths.iter().any(|p| p.starts_with("bin")));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn scan_local_files_empty_directory() {
        let dir =
            std::env::temp_dir().join(format!("diegops-test-sync-empty-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let files = scan_local_files(&dir).unwrap();
        assert!(files.is_empty(), "empty dir should yield no files");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn scan_local_files_nonexistent_directory() {
        let dir =
            std::env::temp_dir().join(format!("diegops-test-sync-noexist-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);

        let files = scan_local_files(&dir).unwrap();
        assert!(files.is_empty(), "nonexistent dir should yield no files");
    }

    #[test]
    fn github_blob_sha_binary_content() {
        // Non-UTF8 binary content
        let content: Vec<u8> = vec![0x00, 0xFF, 0xFE, 0x80, 0x01];
        let sha = github_blob_sha(&content);
        // Should produce a valid 40-char hex SHA
        assert_eq!(sha.len(), 40, "SHA should be 40 hex chars");
        assert!(
            sha.chars().all(|c| c.is_ascii_hexdigit()),
            "SHA should be hex: {sha}"
        );
    }

    #[test]
    fn repo_name_special_characters() {
        assert_eq!(repo_name("user-name"), "diegops-user-name-memory");
        assert_eq!(repo_name("user_name"), "diegops-user_name-memory");
        assert_eq!(repo_name("user123"), "diegops-user123-memory");
        assert_eq!(repo_name("CamelCase"), "diegops-CamelCase-memory");
    }
}
