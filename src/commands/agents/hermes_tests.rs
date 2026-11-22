use super::*;
use std::fs;

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "diegops-hermes-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    fs::create_dir_all(&d).expect("mkdir");
    d
}

fn touch(root: &Path, rel: &str, body: &str) {
    let p = root.join(rel);
    fs::create_dir_all(p.parent().expect("parent")).expect("mkdir");
    fs::write(p, body).expect("write");
}

/// A fake ~/.hermes with a default profile and one named profile.
fn fake_hermes() -> PathBuf {
    let root = tmp("root");
    for base in ["", "profiles/moma/"] {
        touch(&root, &format!("{base}SOUL.md"), "soul");
        touch(&root, &format!("{base}config.yaml"), "model: x");
        touch(&root, &format!("{base}profile.yaml"), "description: x");
        touch(&root, &format!("{base}skills/devops/a/SKILL.md"), "skill");
        touch(&root, &format!("{base}skills/.bundled_manifest"), "m");
        touch(&root, &format!("{base}skills/.usage.json"), "{}");
        touch(&root, &format!("{base}skills/.curator_state"), "{}");
        touch(&root, &format!("{base}skills/.locks/x.lock"), "");
        touch(&root, &format!("{base}cron/jobs.json"), "[]");
        touch(&root, &format!("{base}cron/executions.db"), "db");
        touch(&root, &format!("{base}cron/output/run.txt"), "out");
        touch(&root, &format!("{base}hooks/pre.sh"), "echo");
        touch(&root, &format!("{base}memories/MEMORY.md"), "mem");
        touch(&root, &format!("{base}memories/USER.md"), "user");
        touch(&root, &format!("{base}memories/MEMORY.md.lock"), "");
        touch(&root, &format!("{base}.env"), "K=v");
        touch(&root, &format!("{base}auth.json"), "{}");
        touch(&root, &format!("{base}state.db"), "db");
        touch(&root, &format!("{base}sessions/s.json"), "{}");
        touch(&root, &format!("{base}pairing/p.json"), "{}");
    }
    root
}

#[test]
fn lists_default_and_named_profiles() {
    let root = fake_hermes();
    let names: Vec<String> = profiles(&root)
        .expect("profiles")
        .into_iter()
        .map(|p| p.name)
        .collect();
    assert_eq!(names, vec!["default".to_string(), "moma".to_string()]);
}

#[test]
fn definition_files_exclude_secrets_chats_and_telemetry() {
    let root = fake_hermes();
    let p = &profiles(&root).expect("profiles")[1];
    let files = collect(&p.dir, Set::Definition).expect("collect");
    let rels: Vec<&str> = files.iter().map(|s| s.as_str()).collect();
    assert_eq!(
        rels,
        vec![
            "SOUL.md",
            "config.yaml",
            "cron/jobs.json",
            "hooks/pre.sh",
            "profile.yaml",
            "skills/.bundled_manifest",
            "skills/devops/a/SKILL.md",
        ]
    );
}

#[test]
fn default_profile_does_not_swallow_other_profiles() {
    let root = fake_hermes();
    let p = &profiles(&root).expect("profiles")[0];
    let files = collect(&p.dir, Set::Definition).expect("collect");
    assert!(files.iter().all(|f| !f.starts_with("profiles/")));
}

#[test]
fn memory_set_is_only_memory_markdown() {
    let root = fake_hermes();
    let p = &profiles(&root).expect("profiles")[0];
    let files = collect(&p.dir, Set::Memory).expect("collect");
    assert_eq!(files, vec!["memories/MEMORY.md", "memories/USER.md"]);
}

#[test]
fn sync_copies_and_prunes_only_its_own_set() {
    let root = fake_hermes();
    let p = &profiles(&root).expect("profiles")[1];
    let dest = tmp("dest");
    // Stale definition file and a memory file already in the repo.
    touch(&dest, "skills/old/SKILL.md", "old");
    touch(&dest, "memories/MEMORY.md", "older mem");
    let report =
        sync_set(&p.dir, &dest, Set::Definition, &scan::Allowlist::default()).expect("sync");
    assert!(dest.join("skills/devops/a/SKILL.md").exists());
    assert!(!dest.join("skills/old/SKILL.md").exists(), "stale pruned");
    assert_eq!(
        fs::read_to_string(dest.join("memories/MEMORY.md")).expect("read"),
        "older mem",
        "memory untouched by definition sync"
    );
    assert!(report.skipped.is_empty());
    assert_eq!(report.files, 7);
}

#[test]
fn sync_skips_only_the_file_with_a_secret() {
    let root = fake_hermes();
    let p = &profiles(&root).expect("profiles")[1];
    touch(
        &p.dir,
        "skills/devops/leaky/SKILL.md",
        "token = ghp_abcdefghijklmnopqrstuvwxyz0123456789",
    );
    let dest = tmp("dest-leak");
    let report =
        sync_set(&p.dir, &dest, Set::Definition, &scan::Allowlist::default()).expect("sync");
    assert_eq!(report.skipped.len(), 1);
    assert!(report.skipped[0].contains("skills/devops/leaky/SKILL.md:1"));
    assert!(!dest.join("skills/devops/leaky/SKILL.md").exists());
    assert!(dest.join("skills/devops/a/SKILL.md").exists());
}

#[test]
fn manifest_hashes_round_trip_and_detect_tampering() {
    let root = fake_hermes();
    let p = &profiles(&root).expect("profiles")[1];
    let dest = tmp("dest-verify");
    sync_set(&p.dir, &dest, Set::Definition, &scan::Allowlist::default()).expect("sync");
    let m = manifest(&dest).expect("manifest");
    assert!(verify(&dest, &m).expect("verify").is_empty());
    fs::write(dest.join("SOUL.md"), "tampered").expect("write");
    assert_eq!(
        verify(&dest, &m).expect("verify"),
        vec!["SOUL.md".to_string()]
    );
}

#[test]
fn restore_does_not_clobber_without_force() {
    let root = fake_hermes();
    let p = &profiles(&root).expect("profiles")[1];
    let repo = tmp("repo-restore");
    sync_set(&p.dir, &repo, Set::Definition, &scan::Allowlist::default()).expect("sync");
    fs::write(repo.join("SOUL.md"), "from backup").expect("write");
    // Existing target: refused without force.
    assert!(restore_into(&repo, &p.dir, false).is_err());
    // Forced: overwrites and keeps a .pre-restore copy.
    restore_into(&repo, &p.dir, true).expect("restore");
    assert_eq!(
        fs::read_to_string(p.dir.join("SOUL.md")).expect("r"),
        "from backup"
    );
    assert_eq!(
        fs::read_to_string(p.dir.join("SOUL.md.pre-restore")).expect("r"),
        "soul"
    );
    // Fresh target: fine without force.
    let fresh = tmp("fresh").join("moma");
    restore_into(&repo, &fresh, false).expect("restore fresh");
    assert!(fresh.join("skills/devops/a/SKILL.md").exists());
}

#[test]
fn restore_plan_changes_nothing() {
    let root = fake_hermes();
    let p = &profiles(&root).expect("profiles")[1];
    let repo = tmp("repo-plan");
    sync_set(&p.dir, &repo, Set::Definition, &scan::Allowlist::default()).expect("sync");
    fs::write(repo.join("SOUL.md"), "from backup").expect("write");
    // Existing profile without force: the plan says it would be refused.
    assert!(plan_restore(&repo, &p.dir, false).is_err());
    let plan = plan_restore(&repo, &p.dir, true).expect("plan");
    assert_eq!(plan.replaced, vec!["SOUL.md".to_string()]);
    assert!(plan.unchanged > 0);
    assert_eq!(
        fs::read_to_string(p.dir.join("SOUL.md")).expect("r"),
        "soul"
    );
    assert!(!p.dir.join("SOUL.md.pre-restore").exists());
    let fresh = tmp("fresh-plan").join("moma");
    let plan = plan_restore(&repo, &fresh, false).expect("plan fresh");
    assert!(plan.created > 0 && plan.replaced.is_empty());
    assert!(!fresh.exists(), "a plan must not create the target");
}
