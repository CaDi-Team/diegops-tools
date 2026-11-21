//! `diegops agents hermes` — back up and restore Hermes Agent context.
//!
//! What goes to git (in clear, after the secret scan) per profile
//! (`~/.hermes` = `default`, `~/.hermes/profiles/<name>`):
//! - definition (daily timer): `SOUL.md`, `config.yaml`, `profile.yaml`,
//!   `skills/`, `hooks/`, `skins/`, `plugins/`, `cron/jobs.json`;
//! - memories (reviewed by hand): `memories/MEMORY.md`, `memories/USER.md`.
//!
//! Never: `.env`, `auth.json`, tokens, pairing, `state.db`, sessions, logs,
//! caches. Conversations stay on each PC; secrets live in the secrets
//! backend and are injected as `.env` on restore.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use super::scan;
use super::support::{self, Registry};

type BoxError = Box<dyn std::error::Error>;

/// Directory name of this agent inside the backup repo.
pub const REPO_DIR: &str = "hermes";

/// One Hermes profile on disk.
#[derive(Debug, Clone)]
pub struct Profile {
    /// `default` for the root, else the profile directory name.
    pub name: String,
    /// Profile home.
    pub dir: PathBuf,
}

/// Which part of a profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Set {
    /// Persona, config, skills, hooks, cron definitions.
    Definition,
    /// `MEMORY.md` / `USER.md`.
    Memory,
}

/// Outcome of copying one set.
#[derive(Debug, Default)]
pub struct SyncReport {
    /// Files present in the destination after the sync.
    pub files: usize,
    /// Their total size.
    pub bytes: u64,
    /// `path:line (rule)` of files withheld by the secret scan.
    pub skipped: Vec<String>,
}

/// Profiles under a Hermes root: `default` first, then `profiles/*` sorted.
pub fn profiles(root: &Path) -> Result<Vec<Profile>, BoxError> {
    let mut out = vec![Profile {
        name: "default".to_string(),
        dir: root.to_path_buf(),
    }];
    let dir = root.join("profiles");
    if dir.is_dir() {
        let mut named: Vec<Profile> = fs::read_dir(&dir)?
            .filter_map(Result::ok)
            .filter(|e| e.path().is_dir())
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                (!name.starts_with('.')).then(|| Profile {
                    name,
                    dir: e.path(),
                })
            })
            .collect();
        named.sort_by(|a, b| a.name.cmp(&b.name));
        out.extend(named);
    }
    Ok(out)
}

const DEF_FILES: &[&str] = &["SOUL.md", "config.yaml", "profile.yaml", "cron/jobs.json"];
const DEF_DIRS: &[&str] = &["skills", "hooks", "skins", "plugins"];
const MEM_FILES: &[&str] = &["memories/MEMORY.md", "memories/USER.md"];

/// Files never backed up even inside an included directory.
fn excluded(rel: &str) -> bool {
    let name = rel.rsplit('/').next().unwrap_or(rel);
    rel.split('/')
        .any(|c| c == ".git" || c == ".locks" || c == "__pycache__")
        || name.starts_with(".env")
        || matches!(
            name,
            "auth.json" | ".usage.json" | ".curator_state" | ".curator_ledger.jsonl"
        )
        || [".lock", ".db", "-wal", "-shm", ".pre-restore", ".pyc"]
            .iter()
            .any(|s| name.ends_with(s))
}

/// All regular files under `dir`, as sorted `/`-separated relative paths.
fn walk(dir: &Path) -> Result<Vec<String>, BoxError> {
    fn rec(base: &Path, cur: &Path, out: &mut Vec<String>) -> Result<(), BoxError> {
        if !cur.is_dir() {
            return Ok(());
        }
        for e in fs::read_dir(cur)? {
            let e = e?;
            let ft = e.file_type()?;
            let p = e.path();
            if ft.is_dir() {
                if e.file_name() != ".git" {
                    rec(base, &p, out)?;
                }
            } else if ft.is_file() {
                let rel = p
                    .strip_prefix(base)?
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy().to_string())
                    .collect::<Vec<_>>()
                    .join("/");
                out.push(rel);
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    rec(dir, dir, &mut out)?;
    out.sort();
    Ok(out)
}

fn join_rel(base: &Path, rel: &str) -> PathBuf {
    rel.split('/').fold(base.to_path_buf(), |p, c| p.join(c))
}

/// Relative paths of `set` that exist in profile `dir`, sorted.
pub fn collect(dir: &Path, set: Set) -> Result<Vec<String>, BoxError> {
    let mut out = Vec::new();
    let (files, dirs): (&[&str], &[&str]) = match set {
        Set::Definition => (DEF_FILES, DEF_DIRS),
        Set::Memory => (MEM_FILES, &[]),
    };
    for f in files {
        if join_rel(dir, f).is_file() {
            out.push(f.to_string());
        }
    }
    for d in dirs {
        for rel in walk(&dir.join(d))? {
            let full = format!("{d}/{rel}");
            if !excluded(&full) {
                out.push(full);
            }
        }
    }
    out.sort();
    Ok(out)
}

/// Copies `set` from `src` to `dest`, withholding files with secret hits and
/// pruning files of the same set that no longer exist in `src`.
pub fn sync_set(
    src: &Path,
    dest: &Path,
    set: Set,
    allow: &scan::Allowlist,
) -> Result<SyncReport, BoxError> {
    let mut report = SyncReport::default();
    let wanted = collect(src, set)?;
    for rel in &wanted {
        let bytes = fs::read(join_rel(src, rel))?;
        let hits = scan::scan_bytes(&bytes, allow);
        if let Some(h) = hits.first() {
            report
                .skipped
                .push(format!("{rel}:{} ({})", h.line, h.rule));
            continue;
        }
        let out = join_rel(dest, rel);
        if fs::read(&out).ok().as_deref() != Some(bytes.as_slice()) {
            if let Some(parent) = out.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&out, &bytes)?;
        }
        report.files += 1;
        report.bytes += bytes.len() as u64;
    }
    for rel in collect(dest, set)? {
        if !wanted.contains(&rel) {
            fs::remove_file(join_rel(dest, &rel))?;
        }
    }
    prune_empty_dirs(dest)?;
    Ok(report)
}

fn prune_empty_dirs(dir: &Path) -> Result<bool, BoxError> {
    if !dir.is_dir() {
        return Ok(false);
    }
    let mut empty = true;
    for e in fs::read_dir(dir)? {
        let p = e?.path();
        if p.is_dir() && p.file_name().is_some_and(|n| n != ".git") {
            if prune_empty_dirs(&p)? {
                fs::remove_dir(&p)?;
            } else {
                empty = false;
            }
        } else {
            empty = false;
        }
    }
    Ok(empty)
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// `relative path → sha256` of every file under `dir`.
pub fn manifest(dir: &Path) -> Result<BTreeMap<String, String>, BoxError> {
    let mut m = BTreeMap::new();
    for rel in walk(dir)? {
        m.insert(rel.clone(), sha256_hex(&fs::read(join_rel(dir, &rel))?));
    }
    Ok(m)
}

/// Single digest over a manifest (for evidence: equal digests = equal trees).
pub fn tree_digest(m: &BTreeMap<String, String>) -> String {
    let mut s = String::new();
    for (k, v) in m {
        s.push_str(&format!("{v}  {k}\n"));
    }
    sha256_hex(s.as_bytes())
}

/// Paths in `m` whose file under `dir` is missing or has another hash.
pub fn verify(dir: &Path, m: &BTreeMap<String, String>) -> Result<Vec<String>, BoxError> {
    let mut bad = Vec::new();
    for (rel, h) in m {
        match fs::read(join_rel(dir, rel)) {
            Ok(b) if &sha256_hex(&b) == h => {}
            _ => bad.push(rel.clone()),
        }
    }
    Ok(bad)
}

/// Copies a backed-up profile (`repo_dir`) into `target` and verifies hashes.
/// An existing profile is only overwritten with `force`; replaced files are
/// kept as `<name>.pre-restore`.
pub fn restore_into(repo_dir: &Path, target: &Path, force: bool) -> Result<usize, BoxError> {
    let exists = ["SOUL.md", "config.yaml"]
        .iter()
        .any(|f| target.join(f).exists());
    if exists && !force {
        return Err(format!(
            "{} already has a profile; pass --force to overwrite (a .pre-restore copy is kept)",
            target.display()
        )
        .into());
    }
    let m = manifest(repo_dir)?;
    for rel in m.keys() {
        let bytes = fs::read(join_rel(repo_dir, rel))?;
        let out = join_rel(target, rel);
        if let Ok(old) = fs::read(&out) {
            if old == bytes {
                continue;
            }
            let mut keep = out.clone().into_os_string();
            keep.push(".pre-restore");
            fs::write(PathBuf::from(keep), old)?;
        }
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&out, &bytes)?;
    }
    let bad = verify(target, &m)?;
    if !bad.is_empty() {
        return Err(format!("hash mismatch after restore: {}", bad.join(", ")).into());
    }
    Ok(m.len())
}

// ---------------------------------------------------------------------------
// Orchestration (git + secrets backend)
// ---------------------------------------------------------------------------

/// Resolved settings for the Hermes agent.
pub struct Ctx {
    /// Hermes root (`~/.hermes`).
    pub root: PathBuf,
    /// Local clone of the backup repo.
    pub repo: PathBuf,
    /// Branch override.
    pub branch: Option<String>,
    /// Clone URL.
    pub url: Option<String>,
    /// Secrets base path for `.env` (`<base>/<profile>`).
    pub secrets_path: Option<String>,
    /// Secrets backend override.
    pub secrets_backend: Option<String>,
    /// Heartbeat `<path>#<key>`.
    pub heartbeat_kv: Option<String>,
}

impl Ctx {
    /// Builds the context from the registry.
    pub fn from_registry(reg: &Registry) -> Result<Self, BoxError> {
        let e = reg.find(support::Kind::Hermes).ok_or(
            "hermes is not registered in agents.yaml (add `- kind: hermes` under agents:)",
        )?;
        Ok(Ctx {
            root: support::expand(e.root.as_deref().unwrap_or("~/.hermes")),
            repo: support::expand(&reg.repo.path),
            branch: reg.repo.branch.clone(),
            url: reg.repo.url.clone(),
            secrets_path: e.secrets_path.clone(),
            secrets_backend: e.secrets_backend.clone(),
            heartbeat_kv: reg.heartbeat_kv.clone(),
        })
    }

    fn git(&self) -> super::gitrepo::Repo {
        super::gitrepo::Repo {
            path: self.repo.clone(),
            branch: self.branch.clone(),
            url: self.url.clone(),
        }
    }

    fn repo_profile(&self, name: &str) -> PathBuf {
        self.repo.join(REPO_DIR).join(name)
    }

    fn select(&self, only: Option<&str>) -> Result<Vec<Profile>, BoxError> {
        let all = profiles(&self.root)?;
        match only {
            None => Ok(all),
            Some(n) => {
                let p: Vec<Profile> = all.into_iter().filter(|p| p.name == n).collect();
                if p.is_empty() {
                    Err(format!("no Hermes profile '{n}' under {}", self.root.display()).into())
                } else {
                    Ok(p)
                }
            }
        }
    }

    fn allowlist(&self) -> Result<scan::Allowlist, BoxError> {
        scan::Allowlist::load(&self.repo.join(".secrets-allowlist"))
    }
}

fn host() -> String {
    std::env::var("HOSTNAME")
        .ok()
        .or_else(|| {
            fs::read_to_string("/etc/hostname")
                .ok()
                .map(|s| s.trim().to_string())
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown-host".to_string())
}

/// Copies `set` of the selected profiles, commits and pushes.
fn sync_and_push(
    ctx: &Ctx,
    only: Option<&str>,
    set: Set,
    what: &str,
) -> Result<(usize, u64, String, Vec<String>), BoxError> {
    let git = ctx.git();
    git.ensure()?;
    git.pull()?;
    let allow = ctx.allowlist()?;
    let (mut files, mut bytes, mut skipped) = (0usize, 0u64, Vec::new());
    for p in ctx.select(only)? {
        let dest = ctx.repo_profile(&p.name);
        let r = sync_set(&p.dir, &dest, set, &allow)?;
        let digest = tree_digest(&manifest(&dest)?);
        eprintln!(
            "  {:<16} {:>4} files {:>9} bytes  tree sha256 {}",
            p.name,
            r.files,
            r.bytes,
            &digest[..16]
        );
        files += r.files;
        bytes += r.bytes;
        skipped.extend(r.skipped.into_iter().map(|s| format!("{}/{s}", p.name)));
    }
    let msg = format!("chore(hermes): {what} from {}", host());
    let commit = git.commit_push(&[REPO_DIR], &msg)?;
    Ok((files, bytes, commit, skipped))
}

fn withheld_error(skipped: &[String]) -> BoxError {
    let mut s = format!(
        "{} file(s) withheld by the secret scan (the rest was backed up):",
        skipped.len()
    );
    for k in skipped {
        s.push_str(&format!("\n  {k}"));
    }
    s.push_str(
        "\nRemove the secret from the file (and rotate it if real). A reviewed placeholder \
         can be allowlisted by adding sha256(line) to .secrets-allowlist in the backup repo.",
    );
    s.into()
}

/// `diegops agents hermes backup`: agent definitions → git (memories excluded).
pub fn backup(reg: &Registry, only: Option<&str>) -> Result<(), BoxError> {
    let ctx = Ctx::from_registry(reg)?;
    eprintln!("hermes backup → {}", ctx.repo.display());
    let res = sync_and_push(&ctx, only, Set::Definition, "back up agent context");
    let mut state = support::RunState::load()?;
    let outcome = match &res {
        Ok((files, bytes, commit, skipped)) if skipped.is_empty() => {
            Ok((*files, *bytes, commit.clone()))
        }
        Ok((_, _, _, skipped)) => Err(withheld_error(skipped).to_string()),
        Err(e) => Err(e.to_string()),
    };
    state.record("hermes", outcome.clone(), support::now());
    state.save()?;
    let (files, bytes, commit) = outcome.map_err(BoxError::from)?;
    println!("hermes: backed up {files} files ({bytes} bytes) at {commit}");
    if let Some(kv) = &ctx.heartbeat_kv {
        heartbeat(kv, ctx.secrets_backend.as_deref())?;
    }
    Ok(())
}

fn heartbeat(kv: &str, backend: Option<&str>) -> Result<(), BoxError> {
    let (path, key) = support::split_kv_ref(kv)?;
    let b = crate::secrets_backend::open(backend)?;
    let url = b
        .get(&path)?
        .and_then(|m| m.get(&key).cloned())
        .ok_or_else(|| format!("heartbeat_kv: no key '{key}' at '{path}'"))?;
    ureq::get(url.trim())
        .timeout(std::time::Duration::from_secs(15))
        .call()
        .map_err(|e| format!("heartbeat ping failed: {e}"))?;
    eprintln!("heartbeat sent");
    Ok(())
}

/// Options for [`restore`].
pub struct RestoreOpts<'a> {
    /// Only this profile.
    pub profile: Option<&'a str>,
    /// Restore under this directory instead of the Hermes root.
    pub to: Option<&'a Path>,
    /// Overwrite existing profiles.
    pub force: bool,
    /// Skip `.env` injection.
    pub no_secrets: bool,
    /// Silently skip existing profiles (used by `all pull`).
    pub missing_only: bool,
}

/// `diegops agents hermes restore`.
pub fn restore(reg: &Registry, o: &RestoreOpts) -> Result<(), BoxError> {
    let ctx = Ctx::from_registry(reg)?;
    let git = ctx.git();
    git.ensure()?;
    git.pull()?;
    let base = ctx.repo.join(REPO_DIR);
    let mut names: Vec<String> = match fs::read_dir(&base) {
        Ok(rd) => rd
            .filter_map(Result::ok)
            .filter(|e| e.path().is_dir())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect(),
        Err(_) => Vec::new(),
    };
    names.sort();
    if let Some(p) = o.profile {
        if !names.iter().any(|n| n == p) {
            return Err(format!("profile '{p}' is not in the backup ({})", base.display()).into());
        }
        names.retain(|n| n == p);
    }
    let mut restored = 0;
    for name in &names {
        let target = match (o.to, name.as_str()) {
            (Some(dir), n) => dir.join(n),
            (None, "default") => ctx.root.clone(),
            (None, n) => ctx.root.join("profiles").join(n),
        };
        let src = ctx.repo_profile(name);
        match restore_into(&src, &target, o.force) {
            Ok(n) => {
                let digest = tree_digest(&manifest(&src)?);
                println!(
                    "restored {name}: {n} files → {} (verified, tree sha256 {})",
                    target.display(),
                    &digest[..16]
                );
                restored += 1;
            }
            Err(e) if o.missing_only => {
                eprintln!("skip {name}: already present");
                let _ = e;
                continue;
            }
            Err(e) => return Err(e),
        }
        if !o.no_secrets {
            inject_env(&ctx, name, &target)?;
        }
    }
    eprintln!("{restored} profile(s) restored");
    Ok(())
}

fn inject_env(ctx: &Ctx, profile: &str, target: &Path) -> Result<(), BoxError> {
    let Some(base) = &ctx.secrets_path else {
        eprintln!("  {profile}: no secrets_path configured; .env not injected");
        return Ok(());
    };
    let path = format!("{}/{profile}", base.trim_end_matches('/'));
    let b = crate::secrets_backend::open(ctx.secrets_backend.as_deref())?;
    let Some(map) = b.get(&path)? else {
        eprintln!(
            "  {profile}: nothing at '{path}' ({}); .env not injected",
            b.name()
        );
        return Ok(());
    };
    let body = support::render_env(&map);
    let out = target.join(".env");
    let wrote = super::super::common::write_file_with_backup(&out, body.as_bytes())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&out, fs::Permissions::from_mode(0o600))?;
    }
    eprintln!(
        "  {profile}: .env injected ({} keys from {}){}",
        map.len(),
        b.name(),
        if wrote {
            ", previous kept as .env.bak"
        } else {
            ""
        }
    );
    Ok(())
}

/// `diegops agents hermes memory review`: diff local memories vs the backup.
pub fn memory_review(reg: &Registry, only: Option<&str>) -> Result<(), BoxError> {
    let ctx = Ctx::from_registry(reg)?;
    let mut pending = 0;
    for p in ctx.select(only)? {
        for rel in collect(&p.dir, Set::Memory)? {
            let local = join_rel(&p.dir, &rel);
            let saved = join_rel(&ctx.repo_profile(&p.name), &rel);
            if fs::read(&local).ok() == fs::read(&saved).ok() {
                continue;
            }
            pending += 1;
            println!("=== {}/{rel}", p.name);
            let out = std::process::Command::new("git")
                .args(["diff", "--no-index", "--no-color", "--"])
                .arg(if saved.exists() {
                    saved.as_os_str()
                } else {
                    std::ffi::OsStr::new("/dev/null")
                })
                .arg(&local)
                .output()?;
            print!("{}", String::from_utf8_lossy(&out.stdout));
        }
    }
    if pending == 0 {
        println!("memories: nothing new since the last push");
    } else {
        eprintln!(
            "{pending} memory file(s) changed; push with: diegops agents hermes memory push [--profile X]"
        );
    }
    Ok(())
}

/// `diegops agents hermes memory push`.
pub fn memory_push(reg: &Registry, only: Option<&str>) -> Result<(), BoxError> {
    let ctx = Ctx::from_registry(reg)?;
    let what = match only {
        Some(p) => format!("memories of {p}"),
        None => "memories".to_string(),
    };
    let (files, _, commit, skipped) = sync_and_push(&ctx, only, Set::Memory, &what)?;
    if !skipped.is_empty() {
        return Err(withheld_error(&skipped));
    }
    println!("hermes: {files} memory files at {commit}");
    Ok(())
}

/// `diegops agents hermes secrets push`: profile `.env` → secrets backend.
pub fn secrets_push(reg: &Registry, only: Option<&str>) -> Result<(), BoxError> {
    let ctx = Ctx::from_registry(reg)?;
    let base = ctx
        .secrets_path
        .as_deref()
        .ok_or("set secrets_path for hermes in agents.yaml first")?;
    let b = crate::secrets_backend::open(ctx.secrets_backend.as_deref())?;
    for p in ctx.select(only)? {
        let env = p.dir.join(".env");
        let Ok(text) = fs::read_to_string(&env) else {
            eprintln!("  {:<16} no .env", p.name);
            continue;
        };
        let map = support::parse_env(&text);
        if map.is_empty() {
            eprintln!("  {:<16} .env has no keys", p.name);
            continue;
        }
        for k in map.keys() {
            b.validate_key(k)?;
        }
        let path = format!("{}/{}", base.trim_end_matches('/'), p.name);
        b.put(&path, &map)?;
        println!(
            "  {:<16} {} keys → {path} ({})",
            p.name,
            map.len(),
            b.name()
        );
    }
    Ok(())
}

/// `diegops agents hermes status`.
pub fn status(reg: &Registry) -> Result<(), BoxError> {
    let ctx = Ctx::from_registry(reg)?;
    println!("hermes root: {}", ctx.root.display());
    println!("backup repo: {}", ctx.repo.display());
    print_run(&support::RunState::load()?, "hermes");
    for p in profiles(&ctx.root)? {
        let saved = ctx.repo_profile(&p.name);
        let mut def_pending = 0;
        for rel in collect(&p.dir, Set::Definition)? {
            if fs::read(join_rel(&p.dir, &rel)).ok() != fs::read(join_rel(&saved, &rel)).ok() {
                def_pending += 1;
            }
        }
        let mut mem_pending = 0;
        for rel in collect(&p.dir, Set::Memory)? {
            if fs::read(join_rel(&p.dir, &rel)).ok() != fs::read(join_rel(&saved, &rel)).ok() {
                mem_pending += 1;
            }
        }
        println!(
            "  {:<16} {}  definition changes: {def_pending}  memories to review: {mem_pending}",
            p.name,
            if saved.is_dir() {
                "in backup "
            } else {
                "NOT BACKED"
            }
        );
    }
    Ok(())
}

/// Prints the last-run line of one agent.
pub fn print_run(state: &support::RunState, agent: &str) {
    let now = support::now();
    match state.agents.get(agent) {
        None => println!("{agent}: never backed up by diegops"),
        Some(r) => {
            let ok = r
                .last_ok
                .map(|t| format!("{} ago", support::human_age(t, now)))
                .unwrap_or_else(|| "never".to_string());
            println!(
                "{agent}: last OK {ok} ({} files, {} bytes, {}){}",
                r.files,
                r.bytes,
                r.commit.as_deref().unwrap_or("-"),
                match &r.last_error {
                    Some(e) => format!(
                        "\n  LAST ATTEMPT FAILED {} ago: {}",
                        support::human_age(r.last_attempt, now),
                        e.lines().next().unwrap_or("")
                    ),
                    None => String::new(),
                }
            );
        }
    }
}

#[cfg(test)]
#[path = "hermes_tests.rs"]
mod tests;
