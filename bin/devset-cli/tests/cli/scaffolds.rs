//! Scaffolds: starter files written together once, when the target has none of its own.

use core::fmt::Write as _;
use std::fs;

use crate::sandbox::Sandbox;

/// A profile `book` whose scaffold writes a book unless `docs/book.toml` is there.
fn book(sb: &Sandbox) {
    let scaffold = "scaffold = \"book\"";
    sb.entries(
        "p/book",
        "book",
        &[
            ("docs/book.toml", scaffold, "[book]\n"),
            ("docs/src/SUMMARY.md", scaffold, "# Summary\n"),
            ("theme.css", "", "css\n"),
        ],
    );
    sb.extend("p/book", "[scaffolds.book]\nunless = \"docs/book.toml\"\n");
}

#[test]
fn a_scaffold_is_written_once_and_respected() {
    let sb = Sandbox::new();
    book(&sb);
    let mut log = sb.devset("repo", &["init", "p/book", "--path", "../p"]);
    write!(log, "--- .devset/state.toml\n{}", sb.read("repo/.devset/state.toml")).unwrap();
    fs::remove_file(sb.path("repo/docs/src/SUMMARY.md")).unwrap();
    sb.write("repo/docs/book.toml", "[book]\ntitle = \"Mine\"\n");
    log += &sb.devset("repo", &["apply", "--force"]);
    assert!(!sb.path("repo/docs/src/SUMMARY.md").exists(), "a deleted starter stays deleted");
    log += &sb.devset("repo", &["apply", "--rescaffold", "book/book"]);
    assert!(sb.path("repo/docs/src/SUMMARY.md").exists(), "until the scaffold is asked for again");
    assert_eq!(sb.read("repo/docs/book.toml"), "[book]\ntitle = \"Mine\"\n", "what is there stays");
    log += &sb.devset("repo", &["apply", "--rescaffold", "book/bok"]);
    log += &sb.devset("repo", &["status"]);
    sb.assert(&log, snapbox::file!["snapshots/a_scaffold_is_written_once_and_respected.txt"]);
}

#[test]
fn a_scaffold_finds_the_targets_own() {
    let sb = Sandbox::new();
    book(&sb);
    sb.write("repo/docs/book.toml", "[book]\ntitle = \"Theirs\"\n");
    let mut log = sb.devset("repo", &["init", "p/book", "--path", "../p"]);
    assert!(!sb.path("repo/docs/src/SUMMARY.md").exists(), "the target has its own book");
    fs::remove_file(sb.path("repo/docs/book.toml")).unwrap();
    log += &sb.devset("repo", &["apply"]);
    assert!(!sb.path("repo/docs/book.toml").exists(), "what was decided stays decided");
    log += &sb.devset("repo", &["explain", "docs/src/SUMMARY.md"]);
    write!(log, "--- .devset/state.toml\n{}", sb.read("repo/.devset/state.toml")).unwrap();
    sb.assert(&log, snapbox::file!["snapshots/a_scaffold_finds_the_targets_own.txt"]);
}

#[test]
fn a_changed_answer_moves_no_scaffold() {
    let sb = Sandbox::new();
    sb.entries(
        "p/book",
        "book",
        &[("{{ book_dir }}/book.toml", "scaffold = \"book\"", "[book]\n")],
    );
    sb.extend("p/book", "[vars.book_dir]\ndefault = \"docs\"\n\n[scaffolds.book]\nunless = \"{{ book_dir }}/book.toml\"\n");
    let mut log = sb.devset("repo", &["init", "--path", "../p/book"]);
    log += &sb.devset("repo", &["apply", "--var", "book_dir=site"]);
    assert!(sb.path("repo/docs/book.toml").exists(), "a scaffold written stays where it is");
    assert!(!sb.path("repo/site/book.toml").exists(), "and is not written again");
    log += &sb.devset("repo", &["explain", "site/book.toml"]);
    sb.assert(&log, snapbox::file!["snapshots/a_changed_answer_moves_no_scaffold.txt"]);
}
