//! Sources: named once in the target, their profiles found by name wherever they are.

use core::fmt::Write as _;
use std::fs;

use crate::sandbox::Sandbox;

#[test]
fn sources_are_named_once() {
    let sb = Sandbox::new();
    sb.profile("work/profiles/rust", "rust", &[("rustfmt.toml", "owned", "edition = \"2024\"\n")]);
    sb.profile("work/profiles/book", "book", &[("book.toml", "owned", "[book]\n")]);
    sb.extend(
        "work/profiles/book",
        "[features]\ndefault = [\"katex\"]\nkatex = []\nmermaid = []\n",
    );
    sb.write(
        "work/collection.toml",
        "[collection]\nname = \"acme\"\ndescription = \"Profiles for Rust\"\n",
    );
    sb.release("v1");
    sb.profile("house/deploy", "deploy", &[("deploy.toml", "owned", "deploy\n")]);
    let mut log = sb.devset("repo", &["init", "rust", "--git", "../profiles.git", "--tag", "v1"]);
    log += &sb.devset("repo", &["add", "book"]);
    log += &sb.devset("repo", &["add", "--path", "../house/deploy"]);
    log += &sb.devset("repo", &["add", "mermaid"]);
    write!(log, "--- .devset/config.toml\n{}", sb.read("repo/.devset/config.toml")).unwrap();
    log += &sb.devset("repo", &["list"]);
    log += &sb.devset("repo", &["list", "acme"]);
    log += &sb.devset(".", &["list", "--git", "profiles.git", "--tag", "v1"]);
    sb.assert(&log, snapbox::file!["snapshots/sources_are_named_once.txt"]);
}

#[test]
fn a_source_regroups_its_profiles_without_breaking_a_target() {
    let sb = Sandbox::new();
    sb.profile("p/lint", "lint", &[("lint.toml", "owned", "lint\n")]);
    let mut log = sb.devset("repo", &["init", "p/lint", "--path", "../p"]);
    fs::create_dir_all(sb.path("p/tooling")).unwrap();
    fs::rename(sb.path("p/lint"), sb.path("p/tooling/lint")).unwrap();
    log += &sb.devset("repo", &["status", "--exit-code"]);
    sb.profile("p/twin", "lint", &[]);
    log += &sb.devset("repo", &["status"]);
    sb.assert(
        &log,
        snapbox::file!["snapshots/a_source_regroups_its_profiles_without_breaking_a_target.txt"],
    );
}
