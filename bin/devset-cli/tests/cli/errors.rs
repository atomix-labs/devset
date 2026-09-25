//! Refusals and mistakes, each named with its fix.

use std::fs;
use std::os::unix::fs::symlink;

use crate::sandbox::Sandbox;

#[test]
fn refusals() {
    let sb = Sandbox::new();
    sb.profile("ok", "ok", &[("a.toml", "owned", "a\n")]);
    sb.profile("clash", "clash", &[("a.toml", "owned", "b\n")]);
    sb.profile("fold", "fold", &[("README.md", "owned", "a\n"), ("readme.md", "owned", "b\n")]);
    sb.profile("reserved", "reserved", &[]);
    sb.write(
        "reserved/profile.toml",
        "[profile]\nname = \"reserved\"\n\n[files.\".git/hooks/pre-commit\"]\n",
    );
    sb.profile("missing", "missing", &[]);
    sb.write("missing/profile.toml", "[profile]\nname = \"missing\"\n\n[files.\"gone.toml\"]\n");
    sb.write(
        "typo/profile.toml",
        "[profile]\nname = \"typo\"\n\n[files.\"a\"]\npolcy = \"once\"\n",
    );
    sb.write("newer/profile.toml", "[profile]\nname = \"newer\"\ndevset = \">=99\"\n");

    let mut log = sb.devset(".", &["status"]);
    log += &sb.devset("repo", &["init", "--tag", "v1"]);
    log += &sb.devset("repo", &["init", "--path", "../nope"]);
    for bad in ["reserved", "missing", "typo", "newer", "fold"] {
        log += &sb.devset(&format!("t-{bad}"), &["init", "--path", &format!("../{bad}")]);
    }
    log += &sb.devset("repo", &["init", "--path", "../ok"]);
    log += &sb.devset("repo", &["init", "--path", "../ok"]);
    log += &sb.devset("repo", &["init", "--path", "../clash"]);
    log += &sb.devset("repo/sub", &["init", "--path", "../../ok"]);
    fs::remove_file(sb.path("repo/a.toml")).unwrap();
    symlink("elsewhere", sb.path("repo/a.toml")).unwrap();
    log += &sb.devset("repo", &["status"]);
    sb.assert(&log, snapbox::file!["snapshots/refusals.txt"]);
}

#[test]
fn mistakes_are_named_with_their_fix() {
    let sb = Sandbox::new();
    sb.profile("profiles/rust", "rust", &[("a.toml", "owned", "a\n")]);
    sb.profile("profiles/base", "rust", &[("b.toml", "owned", "b\n")]);
    sb.publish("rust", "rust", &[("a.toml", "owned", "a\n")], "v1");
    sb.profile("t", "t", &[("t.txt", "owned", "hi {{ nmae }}\n")]);
    let manifest = sb.read("t/profile.toml").replace("policy = \"owned\"\n", "template = true\n");
    sb.write("t/profile.toml", &format!("{manifest}\n[vars.name]\ndefault = \"x\"\n"));
    let mut log = sb.devset("r1", &["init", "--path", "../profiles"]);
    log += &sb.devset("r2", &["init", "--path", "../profiles/rust/files"]);
    log += &sb.devset("r3", &["init", "--git", "../profiles.git"]);
    log += &sb.devset("r4", &["init", "--git", "../profiles.git", "--branch", "nope"]);
    log += &sb.devset("r5", &["init", "--path", "../t"]);
    log += &sb.devset("r6", &["init", "--path", ""]);
    sb.devset("r7", &["init", "--path", "../profiles/rust"]);
    log += &sb.devset("r7", &["init", "--path", "../profiles/base"]);
    sb.write(
        "r8/.devset/config.toml",
        "[[layers]]\npath = \"../profiles/rust\"\n\n[merge]\ndriver = \"tool %X %A\"\n",
    );
    log += &sb.devset("r8", &["status"]);
    sb.assert(&log, snapbox::file!["snapshots/mistakes_are_named_with_their_fix.txt"]);
}
