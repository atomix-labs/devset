//! What devset prints: `status` and its JSON, `diff`, the schemas, completions, `--quiet`, and
//! what GitHub Actions shows.

use core::fmt::Write as _;
use std::fs;

use crate::sandbox::Sandbox;

#[test]
fn status_json() {
    let sb = Sandbox::new();
    sb.profile(
        "profile",
        "demo",
        &[("a.toml", "owned", "a = 1\n"), ("b.toml", "merge", "b = 1\n")],
    );
    sb.devset("repo", &["init", "--path", "../profile"]);
    sb.write("repo/a.toml", "a = 2\n");
    sb.assert(
        &sb.devset("repo", &["status", "--json"]),
        snapbox::file!["snapshots/status_json.txt"],
    );
}

#[test]
fn schemas() {
    let sb = Sandbox::new();
    sb.assert(
        &sb.devset(".", &["schema", "profile"]),
        snapbox::file!["snapshots/schema_profile.txt"],
    );
    sb.assert(
        &sb.devset(".", &["schema", "config"]),
        snapbox::file!["snapshots/schema_config.txt"],
    );
}

#[test]
fn quiet_and_github() {
    let sb = Sandbox::new();
    sb.profile("p", "p", &[("a.toml", "owned", "a\n"), ("b.toml", "owned", "b\n")]);
    let mut log = sb.devset("repo", &["-q", "init", "--path", "../p"]);
    sb.write("repo/a.toml", "edited\n");
    sb.profile(
        "p",
        "p",
        &[("a.toml", "owned", "a\n"), ("b.toml", "owned", "b\n"), ("c.toml", "owned", "c\n")],
    );
    sb.write("summary.md", "");
    let root = sb.root.to_str().unwrap().to_owned();
    let summary = sb.path("summary.md");
    let github = [
        ("GITHUB_ACTIONS", "true"),
        ("GITHUB_WORKSPACE", root.as_str()),
        ("GITHUB_STEP_SUMMARY", summary.to_str().unwrap()),
    ];
    log += &sb.devset_with(&github, "repo", &["status", "--exit-code"]);
    write!(log, "--- $GITHUB_STEP_SUMMARY\n{}", sb.read("summary.md")).unwrap();
    sb.assert(&log, snapbox::file!["snapshots/quiet_and_github.txt"]);
}

#[test]
fn diff_shows_how_files_differ() {
    let sb = Sandbox::new();
    sb.profile(
        "p",
        "p",
        &[
            ("a.toml", "owned", "x = 1\n"),
            ("b.toml", "merge", "y = 1\n"),
            ("c.toml", "owned", "z = 1\n"),
            ("same.toml", "owned", "s = 1\n"),
        ],
    );
    sb.write("p/files/logo.png", "PNG\0one");
    let manifest = sb.read("p/profile.toml");
    sb.write("p/profile.toml", &format!("{manifest}\n[files.\"logo.png\"]\n"));
    let mut log = sb.devset("repo", &["init", "--path", "../p"]);
    log += &sb.devset("repo", &["diff"]);
    sb.write("repo/a.toml", "x = 2\n");
    sb.write("repo/b.toml", "y = 1\nw = 0\n");
    fs::remove_file(sb.path("repo/c.toml")).unwrap();
    sb.write("repo/same.toml", "s = 1   \r\n");
    sb.write("repo/logo.png", "PNG\0two");
    log += &sb.devset("repo", &["diff"]);
    log += &sb.devset("repo/sub", &["diff", "../b.toml"]);
    log += &sb.devset("repo", &["diff", "b.tml"]);
    sb.assert(&log, snapbox::file!["snapshots/diff_shows_how_files_differ.txt"]);
}

#[test]
fn completions_are_printed() {
    let sb = Sandbox::new();
    let log = sb.devset(".", &["completions", "bash"]);
    assert!(log.contains("_devset()"), "a bash completion function");
    assert!(log.ends_with("[exit 0]\n"), "that succeeds");
}
