//! Updating: new versions of a profile, pinned by the lock, merged with local edits.

use core::fmt::Write as _;
use std::fs;

use crate::sandbox::Sandbox;

#[test]
fn profile_changes_update_and_release() {
    let sb = Sandbox::new();
    sb.profile(
        "profile",
        "demo",
        &[("a.toml", "owned", "a = 1\n"), ("b.toml", "owned", "b = 1\n")],
    );
    let mut log = sb.devset("repo", &["init", "--path", "../profile"]);
    assert_eq!(sb.blobs("repo"), 2, "one blob per base");

    sb.profile("profile", "demo", &[("a.toml", "owned", "a = 2\n")]);
    log += &sb.devset("repo", &["status"]);
    log += &sb.devset("repo", &["apply"]);
    log += &sb.devset("repo", &["apply"]);
    assert_eq!(sb.read("repo/a.toml"), "a = 2\n", "update");
    assert!(!sb.path("repo/b.toml").exists(), "a released file as devset wrote it goes");
    assert_eq!(sb.blobs("repo"), 1, "unreferenced blobs are collected");
    sb.assert(&log, snapbox::file!["snapshots/profile_changes_update_and_release.txt"]);
}

#[test]
fn git_sources_are_pinned_by_the_lock() {
    let sb = Sandbox::new();
    sb.profile("work/rust", "rust", &[("rustfmt.toml", "owned", "edition = \"2024\"\n")]);
    sb.release("v1");

    let mut log =
        sb.devset("repo", &["init", "--git", "../profiles.git", "--tag", "v1", "--path", "rust"]);
    log += &sb.devset("repo", &["status"]);
    write!(log, "--- lock.toml\n{}", sb.read("repo/.devset/lock.toml")).unwrap();

    // The tag moves upstream; the lock still names the original commit, on any machine.
    sb.profile("work/rust", "rust", &[("rustfmt.toml", "owned", "edition = \"2027\"\n")]);
    sb.git("work", &["commit", "-qam", "v2"]);
    sb.git("work", &["tag", "-fa", "v1", "-m", "moved"]);
    sb.git("work", &["push", "-qf", "../profiles.git", "main", "v1"]);
    fs::remove_dir_all(sb.path("cache")).unwrap();
    fs::remove_file(sb.path("repo/rustfmt.toml")).unwrap();
    log += &sb.devset("repo", &["apply", "--force"]);
    assert_eq!(
        sb.read("repo/rustfmt.toml"),
        "edition = \"2024\"\n",
        "the locked commit, not the moved tag"
    );
    sb.assert(&log, snapbox::file!["snapshots/git_sources_are_pinned_by_the_lock.txt"]);
}

#[test]
fn update_merges_local_edits() {
    let sb = Sandbox::new();
    sb.publish(
        "p",
        "p",
        &[("deny.toml", "merge", "[licenses]\nallow = [\"MIT\"]\n\n[bans]\ndeny = []\n")],
        "v1",
    );
    let mut log =
        sb.devset("repo", &["init", "--git", "../profiles.git", "--branch", "main", "--path", "p"]);
    sb.write("repo/deny.toml", "[licenses]\nallow = [\"MIT\"]\n\n[bans]\ndeny = [\"openssl\"]\n");
    sb.publish(
        "p",
        "p",
        &[("deny.toml", "merge", "[licenses]\nallow = [\"MIT\", \"Zlib\"]\n\n[bans]\ndeny = []\n")],
        "v2",
    );
    log += &sb.devset("repo", &["update", "--dry-run"]);
    log += &sb.devset("repo", &["update"]);
    assert_eq!(
        sb.read("repo/deny.toml"),
        "[licenses]\nallow = [\"MIT\", \"Zlib\"]\n\n[bans]\ndeny = [\"openssl\"]\n",
        "both edits survive"
    );
    log += &sb.devset("repo", &["status"]);
    log += &sb.devset("repo", &["apply"]);
    sb.assert(&log, snapbox::file!["snapshots/update_merges_local_edits.txt"]);
}

#[test]
fn invalid_merges_conflict() {
    let sb = Sandbox::new();
    sb.publish("p", "p", &[("c.toml", "merge", "name = \"app\"\nreplicas = 1\nport = 80\n")], "v1");
    let mut log =
        sb.devset("repo", &["init", "--git", "../profiles.git", "--branch", "main", "--path", "p"]);
    sb.write("repo/c.toml", "name = \"app\"\ntimeout = 30\nreplicas = 1\nport = 80\n");
    sb.publish(
        "p",
        "p",
        &[("c.toml", "merge", "name = \"app\"\nreplicas = 1\nport = 80\ntimeout = 60\n")],
        "v2",
    );
    log += &sb.devset("repo", &["update"]);
    assert_eq!(
        sb.read("repo/c.toml"),
        "name = \"app\"\ntimeout = 30\nreplicas = 1\nport = 80\n",
        "never installed"
    );
    log += &sb.devset("repo", &["update", "--continue"]);
    sb.assert(&log, snapbox::file!["snapshots/invalid_merges_conflict.txt"]);
}

#[test]
fn update_one_layer() {
    let sb = Sandbox::new();
    sb.publish("a", "a", &[("a.txt", "owned", "a1\n")], "v1");
    sb.profile("work/b", "b", &[("b.txt", "owned", "b1\n")]);
    sb.git("work", &["add", "-A"]);
    sb.git("work", &["commit", "-qm", "b"]);
    sb.git("work", &["push", "-q", "../profiles.git", "main"]);
    let mut log = sb.devset("repo", &["init", "--git", "../profiles.git", "--path", "a"]);
    log += &sb.devset("repo", &["init", "--git", "../profiles.git", "--path", "b"]);
    sb.publish("a", "a", &[("a.txt", "owned", "a2\n")], "v2");
    sb.profile("work/b", "b", &[("b.txt", "owned", "b2\n")]);
    sb.git("work", &["commit", "-qam", "b2"]);
    sb.git("work", &["push", "-q", "../profiles.git", "main"]);
    log += &sb.devset("repo", &["update", "nope"]);
    log += &sb.devset("repo", &["update", "b"]);
    assert_eq!(
        (sb.read("repo/a.txt"), sb.read("repo/b.txt")),
        ("a1\n".into(), "b2\n".into()),
        "only b moved"
    );
    sb.assert(&log, snapbox::file!["snapshots/update_one_layer.txt"]);
}

#[test]
fn update_names_are_suggested() {
    let sb = Sandbox::new();
    sb.profile("p", "rust", &[("a.toml", "owned", "a\n")]);
    let mut log = sb.devset("repo", &["init", "--path", "../p"]);
    log += &sb.devset("repo", &["update", "rsut"]);
    sb.assert(&log, snapbox::file!["snapshots/update_names_are_suggested.txt"]);
}
