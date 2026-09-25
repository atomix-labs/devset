//! Applying: the policies, adoption, and the drift gate.

use std::fs;

use crate::sandbox::Sandbox;

#[test]
fn adoption_and_the_drift_gate() {
    let sb = Sandbox::new();
    sb.profile(
        "profile",
        "demo",
        &[
            ("rustfmt.toml", "owned", "max_width = 100\n"),
            ("deny.toml", "merge", "[bans]\n"),
            ("justfile", "once", "default:\n"),
        ],
    );
    sb.write("repo/deny.toml", "[bans]\ndeny = [\"openssl\"]\n");
    let mut log = sb.devset("repo", &["init", "--path", "../profile"]);
    assert_eq!(
        sb.read("repo/deny.toml"),
        "[bans]\ndeny = [\"openssl\"]\n",
        "adoption keeps the file"
    );
    log += &sb.devset("repo", &["status", "--exit-code"]);

    sb.write("repo/rustfmt.toml", "max_width = 80\n");
    log += &sb.devset("repo", &["status", "--exit-code"]);
    log += &sb.devset("repo", &["apply"]);
    assert_eq!(
        sb.read("repo/rustfmt.toml"),
        "max_width = 80\n",
        "plain apply never destroys an edit"
    );
    log += &sb.devset("repo", &["apply", "--force", "--dry-run"]);
    log += &sb.devset("repo", &["apply", "--force"]);
    assert_eq!(sb.read("repo/rustfmt.toml"), "max_width = 100\n", "--force restores owned files");
    log += &sb.devset("repo", &["status", "--exit-code"]);
    sb.assert(&log, snapbox::file!["snapshots/adoption_and_the_drift_gate.txt"]);
}

#[test]
fn cosmetic_edits_are_not_drift() {
    let sb = Sandbox::new();
    sb.profile("profile", "demo", &[("a.toml", "owned", "a = 1\n")]);
    let mut log = sb.devset("repo", &["init", "--path", "../profile"]);
    sb.write("repo/a.toml", "\u{FEFF}a = 1  \r\n\n\n");
    log += &sb.devset("repo", &["status", "--exit-code"]);
    log += &sb.devset("repo", &["apply", "--force"]);
    assert_eq!(
        sb.read("repo/a.toml"),
        "\u{FEFF}a = 1  \r\n\n\n",
        "a reformatted file is left alone"
    );
    sb.assert(&log, snapbox::file!["snapshots/cosmetic_edits_are_not_drift.txt"]);
}

#[test]
fn once_is_written_once() {
    let sb = Sandbox::new();
    sb.profile("profile", "demo", &[("justfile", "once", "v1\n")]);
    let mut log = sb.devset("repo", &["init", "--path", "../profile"]);
    sb.profile("profile", "demo", &[("justfile", "once", "v2\n")]);
    log += &sb.devset("repo", &["apply", "--force"]);
    assert_eq!(sb.read("repo/justfile"), "v1\n", "never rewritten");
    fs::remove_file(sb.path("repo/justfile")).unwrap();
    log += &sb.devset("repo", &["apply", "--force"]);
    assert!(!sb.path("repo/justfile").exists(), "a deletion is respected");
    sb.assert(&log, snapbox::file!["snapshots/once_is_written_once.txt"]);
}

#[test]
fn dry_run_writes_nothing() {
    let sb = Sandbox::new();
    sb.profile("profile", "demo", &[("a.toml", "owned", "a = 1\n")]);
    let log = sb.devset("repo", &["init", "--path", "../profile", "--dry-run"]);
    assert!(!sb.path("repo/.devset").exists(), "no state");
    assert!(!sb.path("repo/a.toml").exists(), "no files");
    sb.assert(&log, snapbox::file!["snapshots/dry_run_writes_nothing.txt"]);
}

#[test]
fn executable_files_are_written_executable() {
    use std::os::unix::fs::PermissionsExt as _;
    let sb = Sandbox::new();
    let files = |script: &'static str| {
        [("setup.sh", "executable = true", script), ("notes.txt", "", "notes\n")]
    };
    sb.entries("tools", "tools", &files("#!/bin/sh\necho ready\n"));
    let mut log = sb.devset("repo", &["init", "--path", "../tools"]);
    let mode = |rel: &str| fs::metadata(sb.path(rel)).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode("repo/setup.sh"), 0o755, "an executable file is written 755");
    assert_eq!(mode("repo/notes.txt") & 0o111, 0, "and no other file is");
    sb.entries("tools", "tools", &files("#!/bin/sh\necho ready again\n"));
    log += &sb.devset("repo", &["apply"]);
    assert_eq!(mode("repo/setup.sh"), 0o755, "and it stays so when written again");
    fs::set_permissions(sb.path("repo/setup.sh"), fs::Permissions::from_mode(0o644)).unwrap();
    log += &sb.devset("repo", &["status", "--exit-code"]);
    sb.assert(&log, snapbox::file!["snapshots/executable_files_are_written_executable.txt"]);
}
