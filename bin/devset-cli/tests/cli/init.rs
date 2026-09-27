//! Starting: a target, a profile or a collection, in a directory given or this one; and `add`
//! starting a target where there is none.

use core::fmt::Write as _;

use crate::sandbox::Sandbox;

#[test]
fn init_starts_a_bare_target() {
    let sb = Sandbox::new();
    let mut log = sb.devset(".", &["init", "hello"]);
    write!(log, "--- hello/.devset/config.toml\n{}", sb.read("hello/.devset/config.toml")).unwrap();
    log += &sb.devset("hello", &["status"]);
    log += &sb.devset("hello", &["init"]);
    log += &sb.devset("third", &["init", "--dry-run"]);
    assert!(!sb.path("third/.devset").exists(), "a dry run writes nothing");
    sb.assert(&log, snapbox::file!["snapshots/init_starts_a_bare_target.txt"]);
}

#[test]
fn init_starts_a_profile_or_a_collection() {
    let sb = Sandbox::new();
    let mut log = sb.devset(".", &["init", "my-lint", "--profile"]);
    write!(log, "--- my-lint/profile.toml\n{}", sb.read("my-lint/profile.toml")).unwrap();
    log += &sb.devset(".", &["init", "my-lint", "--profile"]);
    log += &sb.devset("here", &["init", "--profile"]);
    assert!(sb.path("here/profile.toml").exists(), "in the directory devset runs in");
    log += &sb.devset(".", &["init", "acme", "--collection"]);
    log += &sb.devset(".", &["list", "--path", "acme"]);
    log += &sb.devset(".", &["init", "bad.name", "--profile"]);
    sb.assert(&log, snapbox::file!["snapshots/init_starts_a_profile_or_a_collection.txt"]);
}
