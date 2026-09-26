//! Starting: a target in a new directory or this one, and a profile or a collection to author.

use core::fmt::Write as _;

use crate::sandbox::Sandbox;

#[test]
fn new_starts_a_bare_target() {
    let sb = Sandbox::new();
    sb.profile("p/base", "base", &[("a.toml", "owned", "a\n")]);
    let mut log = sb.devset(".", &["new", "hello"]);
    write!(log, "--- hello/.devset/config.toml\n{}", sb.read("hello/.devset/config.toml")).unwrap();
    log += &sb.devset("hello", &["status"]);
    log += &sb.devset("hello", &["add", "p/base", "--path", "../p"]);
    log += &sb.devset(".", &["new", "second", "p/base", "--path", "p"]);
    assert!(
        sb.read("second/.devset/config.toml").contains("p = { path = \"../p\" }"),
        "from the target"
    );
    sb.git("p", &["init", "-q", "-b", "main"]);
    sb.git("p", &["add", "-A"]);
    sb.git("p", &["commit", "-qm", "profiles"]);
    log += &sb.devset(".", &["new", "fourth", "p/base", "--git", "p", "--branch", "main"]);
    assert!(sb.path("fourth/a.toml").exists(), "a local repository, from where devset ran");
    log += &sb.devset("third", &["init", "--dry-run"]);
    assert!(!sb.path("third/.devset").exists(), "a dry run writes nothing");
    sb.assert(&log, snapbox::file!["snapshots/new_starts_a_bare_target.txt"]);
}

#[test]
fn new_starts_a_profile_or_a_collection() {
    let sb = Sandbox::new();
    let mut log = sb.devset(".", &["new", "--profile", "my-lint"]);
    write!(log, "--- my-lint/profile.toml\n{}", sb.read("my-lint/profile.toml")).unwrap();
    log += &sb.devset(".", &["new", "--profile", "my-lint"]);
    log += &sb.devset(".", &["new", "--collection", "acme"]);
    log += &sb.devset(".", &["list", "--path", "acme"]);
    log += &sb.devset(".", &["new", "demo", "acme/example", "--path", "acme"]);
    log += &sb.devset("demo", &["status"]);
    log += &sb.devset(".", &["new", "--profile", "bad.name"]);
    sb.assert(&log, snapbox::file!["snapshots/new_starts_a_profile_or_a_collection.txt"]);
}
