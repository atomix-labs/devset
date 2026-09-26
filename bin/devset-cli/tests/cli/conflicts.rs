//! Conflicts: waiting in sidecars, updates withheld, and taking an update back.

use core::fmt::Write as _;
use std::fs;

use crate::sandbox::Sandbox;

#[test]
fn conflicts_wait_in_sidecars() {
    let sb = Sandbox::new();
    sb.publish("p", "p", &[("a.toml", "merge", "x = 1\n"), ("b.toml", "owned", "b = 1\n")], "v1");
    let mut log =
        sb.devset("repo", &["init", "--git", "../profiles.git", "--branch", "main", "--path", "p"]);
    sb.write("repo/a.toml", "x = 2\n");
    sb.publish("p", "p", &[("a.toml", "merge", "x = 3\n"), ("b.toml", "owned", "b = 2\n")], "v2");
    let github = [("GITHUB_ACTIONS", "true")];
    log += &sb.devset_with(&github, "repo", &["update", "--dry-run"]);
    assert!(!sb.path("repo/.devset/conflicts").exists(), "a dry run writes no sidecar");
    log += &sb.devset("repo", &["update"]);
    assert_eq!(sb.read("repo/a.toml"), "x = 2\n", "the working file is untouched");
    assert_eq!(sb.read("repo/b.toml"), "b = 2\n", "clean files are applied");
    log += &sb.devset("repo", &["status", "--exit-code"]);
    log += &sb.devset("repo", &["apply"]);
    log += &sb.devset("repo", &["update", "--continue"]);
    sb.write("repo/.devset/conflicts/a.toml", "x = 23\n");
    log += &sb.devset("repo", &["update", "--continue"]);
    assert_eq!(sb.read("repo/a.toml"), "x = 23\n", "the resolution is installed");
    assert!(!sb.path("repo/.devset/conflicts").exists(), "sidecars are cleared");
    log += &sb.devset("repo", &["update", "--continue"]);
    sb.assert(&log, snapbox::file!["snapshots/conflicts_wait_in_sidecars.txt"]);
}

#[test]
fn apply_none_withholds_everything() {
    let sb = Sandbox::new();
    sb.publish("p", "p", &[("a.toml", "merge", "x = 1\n"), ("b.toml", "owned", "b = 1\n")], "v1");
    let mut log =
        sb.devset("repo", &["init", "--git", "../profiles.git", "--branch", "main", "--path", "p"]);
    sb.write(
        "repo/.devset/config.toml",
        &format!(
            "{}\n[merge]\non-conflict = \"apply-none\"\n",
            sb.read("repo/.devset/config.toml")
        ),
    );
    sb.write("repo/a.toml", "x = 2\n");
    sb.publish("p", "p", &[("a.toml", "merge", "x = 3\n"), ("b.toml", "owned", "b = 2\n")], "v2");
    log += &sb.devset("repo", &["update", "--dry-run"]);
    assert!(!sb.path("repo/.devset/conflicts").exists(), "a dry run withholds nothing");
    log += &sb.devset("repo", &["update"]);
    assert_eq!(sb.read("repo/b.toml"), "b = 1\n", "nothing else is written");
    assert!(
        sb.path("repo/.devset/conflicts/.devset/pending.toml").exists(),
        "the update waits in its pending lock"
    );
    sb.write("repo/.devset/conflicts/a.toml", "x = 23\n");
    log += &sb.devset("repo", &["update", "--continue"]);
    assert_eq!(
        (sb.read("repo/a.toml"), sb.read("repo/b.toml")),
        ("x = 23\n".into(), "b = 2\n".into()),
        "all applied"
    );
    assert!(!sb.path("repo/.devset/conflicts").exists(), "pending lock removed");
    sb.assert(&log, snapbox::file!["snapshots/apply_none_withholds_everything.txt"]);
}

#[test]
fn abort_takes_the_update_back() {
    let sb = Sandbox::new();
    sb.publish("p", "p", &[("a.toml", "merge", "x = 1\n"), ("b.toml", "owned", "b = 1\n")], "v1");
    let mut log =
        sb.devset("repo", &["init", "--git", "../profiles.git", "--branch", "main", "--path", "p"]);
    sb.write("repo/a.toml", "x = 2\n");
    let records = || (sb.read("repo/.devset/lock.toml"), sb.read("repo/.devset/state.toml"));
    let (before, bases) = (records(), sb.blobs("repo"));
    sb.publish(
        "p",
        "p",
        &[
            ("a.toml", "merge", "x = 3\n"),
            ("b.toml", "owned", "b = 2\n"),
            ("c.toml", "owned", "c = 1\n"),
        ],
        "v2",
    );
    log += &sb.devset("repo", &["update"]);
    log += &sb.devset("repo", &["update", "--abort", "--dry-run"]);
    sb.write("repo/b.toml", "b = 9\n");
    log += &sb.devset("repo", &["update", "--abort"]);
    assert_eq!(sb.read("repo/b.toml"), "b = 9\n", "a refused abort writes nothing");
    log += &sb.devset("repo", &["update", "--abort", "--force"]);
    assert_eq!(
        (sb.read("repo/a.toml"), sb.read("repo/b.toml")),
        ("x = 2\n".into(), "b = 1\n".into()),
        "every file as before the update"
    );
    assert!(!sb.path("repo/c.toml").exists(), "a file the update created is removed");
    assert_eq!((records(), sb.blobs("repo")), (before, bases), "and the lock, state and bases");
    assert!(!sb.path("repo/.devset/conflicts").exists(), "the conflicts are discarded");
    log += &sb.devset("repo", &["update", "--abort"]);
    sb.assert(&log, snapbox::file!["snapshots/abort_takes_the_update_back.txt"]);
}

#[test]
fn abort_discards_a_withheld_update() {
    let sb = Sandbox::new();
    sb.publish("p", "p", &[("a.toml", "merge", "x = 1\n"), ("b.toml", "owned", "b = 1\n")], "v1");
    let mut log =
        sb.devset("repo", &["init", "--git", "../profiles.git", "--branch", "main", "--path", "p"]);
    let config = sb.read("repo/.devset/config.toml");
    sb.write(
        "repo/.devset/config.toml",
        &format!("{config}\n[merge]\non-conflict = \"apply-none\"\n"),
    );
    sb.write("repo/a.toml", "x = 2\n");
    let lock = sb.read("repo/.devset/lock.toml");
    sb.publish("p", "p", &[("a.toml", "merge", "x = 3\n"), ("b.toml", "owned", "b = 2\n")], "v2");
    log += &sb.devset("repo", &["update"]);
    log += &sb.devset("repo", &["update", "--abort"]);
    assert_eq!(sb.read("repo/.devset/lock.toml"), lock, "the lock never moved");
    assert!(!sb.path("repo/.devset/conflicts").exists(), "the withheld lock is discarded");
    log += &sb.devset("repo", &["status"]);
    sb.assert(&log, snapbox::file!["snapshots/abort_discards_a_withheld_update.txt"]);
}

#[test]
fn unfinished_updates_fail_the_gate() {
    let sb = Sandbox::new();
    sb.publish("p", "p", &[("a.toml", "merge", "x = 1\n"), ("b.toml", "owned", "b = 1\n")], "v1");
    sb.devset("repo", &["init", "--git", "../profiles.git", "--branch", "main", "--path", "p"]);
    let config = sb.read("repo/.devset/config.toml");
    sb.write(
        "repo/.devset/config.toml",
        &format!("{config}\n[merge]\non-conflict = \"apply-none\"\n"),
    );
    sb.write("repo/a.toml", "x = 2\n");
    sb.publish("p", "p", &[("a.toml", "merge", "x = 3\n"), ("b.toml", "owned", "b = 2\n")], "v2");
    sb.devset("repo", &["update"]);
    let log = sb.devset("repo", &["status", "--exit-code"]);
    sb.assert(&log, snapbox::file!["snapshots/unfinished_updates_fail_the_gate.txt"]);
}

#[test]
fn conflicts_without_a_merge() {
    let sb = Sandbox::new();
    sb.profile("p", "p", &[("m.toml", "merge", "a = 1\n"), ("o.toml", "owned", "o = 1\n")]);
    sb.write("p/files/b.dat", "B\0one");
    let manifest = sb.read("p/profile.toml");
    sb.write("p/profile.toml", &format!("{manifest}\n[files.\"b.dat\"]\npolicy = \"merge\"\n"));
    let mut log = sb.devset("repo", &["init", "--path", "../p"]);
    // A base store that was never committed, then cloned: the bases are gone.
    fs::remove_dir_all(sb.path("repo/.devset/base")).unwrap();
    sb.write("repo/m.toml", "a = 1\nb = 2\n");
    sb.write("p/files/m.toml", "z = 0\na = 1\n");
    sb.write("repo/b.dat", "B\0mine");
    sb.write("p/files/b.dat", "B\0theirs");
    sb.write("p/files/o.toml", "o = 2\n");
    log += &sb.devset("repo", &["apply"]);
    assert_eq!(sb.read("repo/o.toml"), "o = 2\n", "every other file proceeds");
    assert_eq!(sb.read("repo/.devset/conflicts/b.dat"), "B\0theirs", "the profile's version");
    write!(log, "--- .devset/conflicts/m.toml\n{}", sb.read("repo/.devset/conflicts/m.toml"))
        .unwrap();
    sb.assert(&log, snapbox::file!["snapshots/conflicts_without_a_merge.txt"]);
}

#[test]
fn a_layer_added_while_conflicts_are_withheld_stays() {
    let sb = Sandbox::new();
    sb.publish("p", "p", &[("a.toml", "merge", "x = 1\n")], "v1");
    sb.profile("work/q", "q", &[("q.toml", "owned", "q = 1\n")]);
    sb.release("v2");
    let mut log =
        sb.devset("repo", &["init", "--git", "../profiles.git", "--tag", "v1", "--path", "p"]);
    let config = sb.read("repo/.devset/config.toml");
    sb.write(
        "repo/.devset/config.toml",
        &format!("{config}\n[merge]\non-conflict = \"apply-none\"\n"),
    );
    sb.write("repo/a.toml", "x = 2\n");
    sb.publish("p", "p", &[("a.toml", "merge", "x = 3\n")], "v3");
    let config = sb.read("repo/.devset/config.toml").replace("tag = \"v1\"", "tag = \"v3\"");
    sb.write("repo/.devset/config.toml", &config);
    log +=
        &sb.devset("repo", &["add", "q", "--git", "../profiles.git", "--tag", "v3", "--path", "q"]);
    assert!(sb.read("repo/.devset/config.toml").contains("q/q"), "the layer is the target's");
    sb.write("repo/.devset/conflicts/a.toml", "x = 23\n");
    log += &sb.devset("repo", &["update", "--continue"]);
    assert!(sb.path("repo/q.toml").exists(), "and applied once the conflict is resolved");
    sb.assert(
        &log,
        snapbox::file!["snapshots/a_layer_added_while_conflicts_are_withheld_stays.txt"],
    );
}
