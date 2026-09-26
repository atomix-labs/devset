//! Starters: a file absent from the target starts from one, and every part is spliced into it in
//! the same run.

use core::fmt::Write as _;
use std::fs;

use crate::sandbox::Sandbox;

/// A source `p`: `workspace` scaffolds `Cargo.toml`, and `lints` and `release` own keys of it.
fn source(sb: &Sandbox) {
    let keys = "scope = \"keys\"";
    sb.entries(
        "p/workspace",
        "workspace",
        &[("Cargo.toml", "policy = \"once\"\ntemplate = true", "[workspace]\nmembers = [\"crates/*\"]\n\n[workspace.package]\nname = \"{{ project }}\"\n")],
    );
    sb.extend("p/workspace", "[vars.project]\ndefault = \"hello\"\n");
    sb.entries(
        "p/lints",
        "lints",
        &[("Cargo.toml", keys, "[workspace.lints.rust]\nunsafe_code = \"deny\"\n")],
    );
    sb.entries(
        "p/release",
        "release",
        &[("Cargo.toml", keys, "[profile.release]\nlto = \"fat\"\n")],
    );
    sb.profile("p/rust", "rust", &[]);
    sb.extend("p/rust", "[requires]\nworkspace = {}\nlints = {}\nrelease = {}\n");
}

#[test]
fn a_starter_composes_with_parts_in_one_run() {
    let sb = Sandbox::new();
    source(&sb);
    let mut log = sb.devset("repo", &["init", "p/rust", "--path", "../p"]);
    write!(log, "--- Cargo.toml\n{}", sb.read("repo/Cargo.toml")).unwrap();
    log += &sb.devset("repo", &["status", "-v"]);
    sb.write("mine/Cargo.toml", "[package]\nname = \"mine\"\n");
    log += &sb.devset("mine", &["init", "p/rust", "--path", "../p"]);
    let mine = sb.read("mine/Cargo.toml");
    assert!(
        mine.starts_with("[package]\nname = \"mine\"\n"),
        "the target's own file stays: {mine}"
    );
    assert!(!mine.contains("[workspace]\n"), "and never takes the starter");
    write!(log, "--- mine/Cargo.toml\n{mine}").unwrap();
    sb.assert(&log, snapbox::file!["snapshots/a_starter_composes_with_parts_in_one_run.txt"]);
}

#[test]
fn a_part_starts_from_its_own_starter() {
    let sb = Sandbox::new();
    sb.entries(
        "p/book",
        "book",
        &[(
            "docs/book.toml",
            "scope = \"keys\"\nstarter = \"book.starter.toml\"",
            "[output.html]\nmathjax-support = false\n",
        )],
    );
    sb.write("p/book/files/book.starter.toml", "[book]\ntitle = \"Book\"\n");
    let mut log = sb.devset("repo", &["init", "--path", "../p/book"]);
    write!(log, "--- docs/book.toml\n{}", sb.read("repo/docs/book.toml")).unwrap();
    sb.write("mine/docs/book.toml", "[book]\ntitle = \"Mine\"\n");
    log += &sb.devset("mine", &["init", "--path", "../p/book"]);
    write!(log, "--- mine/docs/book.toml\n{}", sb.read("mine/docs/book.toml")).unwrap();
    sb.assert(&log, snapbox::file!["snapshots/a_part_starts_from_its_own_starter.txt"]);
}

#[test]
fn a_path_takes_its_variables() {
    let sb = Sandbox::new();
    sb.entries("p/book", "book", &[("{{ book_dir }}/book.toml", "", "[book]\n")]);
    sb.extend("p/book", "[vars.book_dir]\ndefault = \"docs\"\n");
    let mut log = sb.devset("repo", &["init", "--path", "../p/book"]);
    assert!(sb.path("repo/docs/book.toml").exists(), "the default");
    log += &sb.devset("repo", &["apply", "--var", "book_dir=site"]);
    assert!(sb.path("repo/site/book.toml").exists(), "a new answer moves the file");
    assert!(!sb.path("repo/docs").exists(), "and takes the old one, and its directory");
    log += &sb.devset("repo", &["apply", "--var", "book_dir=../out"]);
    sb.assert(&log, snapbox::file!["snapshots/a_path_takes_its_variables.txt"]);
}

#[test]
fn from_picks_a_starter_and_keeps_every_part() {
    let sb = Sandbox::new();
    let once = "policy = \"once\"";
    sb.entries("p/ws1", "ws1", &[("Cargo.toml", once, "[workspace]\nmembers = [\"one/*\"]\n")]);
    sb.entries("p/ws2", "ws2", &[("Cargo.toml", once, "[workspace]\nmembers = [\"two/*\"]\n")]);
    sb.entries(
        "p/lints",
        "lints",
        &[("Cargo.toml", "scope = \"keys\"", "[workspace.lints.rust]\nunsafe_code = \"deny\"\n")],
    );
    let mut log = sb.devset("repo", &["init", "p/lints", "--path", "../p"]);
    log += &sb.devset("repo", &["add", "p/ws1"]);
    log += &sb.devset("repo", &["add", "p/ws2"]);
    let config = sb.read("repo/.devset/config.toml");
    let config = format!(
        "{config}\n[[layers]]\nprofile = \"p/ws2\"\n\n[files.\"Cargo.toml\"]\nfrom = \"ws1\"\n"
    );
    sb.write("repo/.devset/config.toml", &config);
    log += &sb.devset("repo", &["apply"]);
    let cargo = sb.read("repo/Cargo.toml");
    assert!(cargo.contains("unsafe_code"), "the parts stay: {cargo}");
    write!(log, "--- Cargo.toml\n{cargo}").unwrap();
    log += &sb.devset("repo", &["status", "--exit-code"]);
    sb.assert(&log, snapbox::file!["snapshots/from_picks_a_starter_and_keeps_every_part.txt"]);
}

#[test]
fn a_starter_written_again_takes_its_parts() {
    let sb = Sandbox::new();
    sb.entries("p/ws", "ws", &[("Cargo.toml", "scaffold = \"ws\"", "[workspace]\nmembers = []\n")]);
    sb.extend("p/ws", "[scaffolds.ws]\nunless = \"Cargo.toml\"\n");
    sb.entries(
        "p/lints",
        "lints",
        &[("Cargo.toml", "scope = \"keys\"", "[workspace.lints.rust]\nunsafe_code = \"deny\"\n")],
    );
    sb.profile("p/rust", "rust", &[]);
    sb.extend("p/rust", "[requires]\nws = {}\nlints = {}\n");
    let mut log = sb.devset("repo", &["init", "p/rust", "--path", "../p"]);
    fs::remove_file(sb.path("repo/Cargo.toml")).unwrap();
    log += &sb.devset("repo", &["apply"]);
    assert!(!sb.path("repo/Cargo.toml").exists(), "a deleted starter stays deleted");
    log += &sb.devset("repo", &["apply", "--rescaffold", "ws/ws"]);
    let cargo = sb.read("repo/Cargo.toml");
    assert!(cargo.contains("[workspace]") && cargo.contains("unsafe_code"), "whole again: {cargo}");
    log += &sb.devset("repo", &["status", "--exit-code"]);
    sb.assert(&log, snapbox::file!["snapshots/a_starter_written_again_takes_its_parts.txt"]);
}

#[test]
fn two_starters_are_settled_by_the_target() {
    let sb = Sandbox::new();
    for name in ["zeta", "alpha"] {
        let entry = "scope = \"keys\"\nstarter = \"start.toml\"";
        sb.entries(
            &format!("p/{name}"),
            name,
            &[("book.toml", entry, &format!("[{name}]\non = true\n"))],
        );
        sb.write(&format!("p/{name}/files/start.toml"), &format!("[book]\ntitle = \"{name}'s\"\n"));
    }
    let mut log = sb.devset("repo", &["init", "p/zeta", "--path", "../p"]);
    log += &sb.devset("repo", &["add", "p/alpha"]);
    let config = sb.read("repo/.devset/config.toml");
    let config = format!(
        "{config}\n[[layers]]\nprofile = \"p/alpha\"\n\n[files.\"book.toml\"]\nfrom = \"alpha\"\n"
    );
    sb.write("repo/.devset/config.toml", &config);
    fs::remove_file(sb.path("repo/book.toml")).unwrap();
    log += &sb.devset("repo", &["apply", "--force"]);
    let book = sb.read("repo/book.toml");
    assert!(
        book.contains("alpha's") && book.contains("[zeta]"),
        "alpha starts it, zeta joins: {book}"
    );
    write!(log, "--- book.toml\n{book}").unwrap();
    sb.assert(&log, snapbox::file!["snapshots/two_starters_are_settled_by_the_target.txt"]);
}
