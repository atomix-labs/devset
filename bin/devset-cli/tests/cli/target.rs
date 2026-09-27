//! The target: found from anywhere inside it, started by `add` where there is none, with its
//! state in `.devset/`.

use core::fmt::Write as _;

use crate::sandbox::Sandbox;

#[test]
fn worktrees_and_subdirectories_find_their_target() {
    let sb = Sandbox::new();
    sb.profile("profile", "demo", &[("a.toml", "owned", "a = 1\n")]);
    sb.git("repo", &["init", "-q", "-b", "main"]);
    let mut log = sb.devset("repo", &["init", "--path", "../profile"]);
    sb.git("repo", &["add", "-A"]);
    sb.git("repo", &["commit", "-qm", "adopt devset"]);
    sb.git("repo", &["worktree", "add", "-q", "../wt"]);
    log += &sb.devset("wt/.devset", &["status", "--exit-code"]);
    sb.assert(&log, snapbox::file!["snapshots/worktrees_and_subdirectories_find_their_target.txt"]);
}

#[test]
fn state_lives_in_dot_devset() {
    let sb = Sandbox::new();
    sb.profile("base", "base", &[("a.toml", "owned", "a = 1\n")]);
    sb.profile("extra", "extra", &[("b.toml", "owned", "b = 1\n")]);
    let mut log = sb.devset("repo", &["init", "--path", "../base"]);
    let config = sb.read("repo/.devset/config.toml");
    sb.write("repo/.devset/config.toml", &format!("# layers, in order\n{config}"));
    log += &sb.devset("repo", &["init", "--path", "../extra"]);
    for file in ["config.toml", "state.toml", ".gitignore", ".gitattributes"] {
        write!(log, "--- .devset/{file}\n{}", sb.read(&format!("repo/.devset/{file}"))).unwrap();
    }
    sb.assert(&log, snapbox::file!["snapshots/state_lives_in_dot_devset.txt"]);
}

#[test]
fn add_starts_a_target_where_there_is_none() {
    let sb = Sandbox::new();
    sb.profile("p/base", "base", &[("a.toml", "owned", "a\n")]);
    let mut log = sb.devset("empty", &["add", "p/base", "--path", "../p"]);
    assert!(sb.path("empty/a.toml").exists(), "an empty directory");
    sb.git("repo", &["init", "-q", "-b", "main"]);
    sb.write("repo/README.md", "# repo\n");
    log += &sb.devset("repo", &["add", "p/base", "--path", "../p"]);
    assert!(sb.path("repo/a.toml").exists(), "a repository's top level");
    sb.write("loose/notes.txt", "mine\n");
    log += &sb.devset("loose", &["add", "p/base", "--path", "../p"]);
    assert!(!sb.path("loose/.devset").exists(), "anywhere else, nothing is written");
    log += &sb.devset("loose", &["init"]);
    log += &sb.devset("loose", &["add", "p/base", "--path", "../p"]);
    sb.git("p", &["init", "-q", "-b", "main"]);
    sb.git("p", &["add", "-A"]);
    sb.git("p", &["commit", "-qm", "profiles"]);
    log += &sb.devset("fourth", &["add", "p/base", "--git", "../p", "--branch", "main"]);
    log += &sb.devset("fifth", &["add", "p/base", "--path", "../p", "--dry-run"]);
    assert!(!sb.path("fifth/.devset").exists(), "a dry run writes nothing");
    sb.assert(&log, snapbox::file!["snapshots/add_starts_a_target_where_there_is_none.txt"]);
}
