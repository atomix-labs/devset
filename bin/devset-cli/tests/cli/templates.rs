//! Templates: files rendered with the answers to a profile's variables.

use core::fmt::Write as _;

use crate::sandbox::Sandbox;

#[test]
fn templates_render_answers() {
    let sb = Sandbox::new();
    sb.profile(
        "p",
        "p",
        &[
            ("Cargo.toml", "once", "authors = [\"{{ author }}\"]\n"),
            ("cfg.toml", "owned", "cpu = \"{{ cpu }}\"\n"),
        ],
    );
    let manifest = sb
        .read("p/profile.toml")
        .replace("policy = \"once\"\n", "policy = \"once\"\ntemplate = true\n");
    let manifest = manifest.replace(
        "[files.\"cfg.toml\"]\npolicy = \"owned\"\n",
        "[files.\"cfg.toml\"]\npolicy = \"owned\"\ntemplate = true\n",
    );
    sb.write("p/profile.toml", &format!("{manifest}\n[vars.author]\nprompt = \"Author name\"\n\n[vars.cpu]\ndefault = \"native\"\n"));
    let mut log = sb.devset("repo", &["init", "--path", "../p"]);
    assert!(!sb.path("repo/.devset").exists(), "nothing written while an answer is missing");
    log += &sb.devset("repo", &["init", "--path", "../p", "--var", "auther=Ada"]);
    log += &sb.devset("repo", &["init", "--path", "../p", "--var", "author=Ada"]);
    assert_eq!(sb.read("repo/Cargo.toml"), "authors = [\"Ada\"]\n", "rendered");
    write!(log, "--- .devset/answers.toml\n{}", sb.read("repo/.devset/answers.toml")).unwrap();
    log += &sb.devset("repo", &["apply", "--var", "cpu=x86-64-v3"]);
    assert_eq!(
        sb.read("repo/cfg.toml"),
        "cpu = \"x86-64-v3\"\n",
        "a new answer updates the rendered file"
    );
    log += &sb.devset("repo", &["status"]);
    sb.assert(&log, snapbox::file!["snapshots/templates_render_answers.txt"]);
}

#[test]
fn dropped_answers_are_noted() {
    let sb = Sandbox::new();
    sb.profile("p", "p", &[("t.txt", "owned", "{{ cpu }}\n")]);
    let manifest = sb.read("p/profile.toml").replace("policy = \"owned\"\n", "template = true\n");
    sb.write("p/profile.toml", &format!("{manifest}\n[vars.cpu]\ndefault = \"x86-64-v2\"\n"));
    sb.profile("q", "q", &[("q.txt", "owned", "q\n")]);
    sb.devset("repo", &["init", "--path", "../p"]);
    let mut log = sb.devset("repo", &["init", "--path", "../q"]);
    log += &sb.devset("repo", &["remove", "p"]);
    assert!(!sb.path("repo/.devset/answers.toml").exists(), "no answers left");
    sb.assert(&log, snapbox::file!["snapshots/dropped_answers_are_noted.txt"]);
}
