//! The target: found from anywhere inside it, with its state in `.devset/`.

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
