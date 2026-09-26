//! Gates: entries that apply only when their `when` holds, settled in one run.

use crate::sandbox::Sandbox;

#[test]
fn variables_and_profiles_gate_entries() {
    let sb = Sandbox::new();
    let mit = "when = { vars = { license = [\"MIT\", \"MIT OR Apache-2.0\"] } }";
    let apache = "when = { vars = { license = [\"Apache-2.0\", \"MIT OR Apache-2.0\"] } }";
    let linted = "when = { profiles = [\"lint\"] }";
    sb.entries(
        "p/project",
        "project",
        &[
            ("LICENSE-MIT", mit, "MIT\n"),
            ("LICENSE-APACHE", apache, "Apache\n"),
            ("lint.md", linted, "lint\n"),
        ],
    );
    sb.extend("p/project", "[vars.license]\ndefault = \"MIT\"\n");
    sb.profile("p/lint", "lint", &[("lint.toml", "owned", "lint\n")]);
    let mut log = sb.devset("repo", &["init", "p/project", "--path", "../p"]);
    assert!(!sb.path("repo/LICENSE-APACHE").exists(), "the answer picks the licence");
    log += &sb.devset("repo", &["apply", "--var", "license=MIT OR Apache-2.0"]);
    assert!(sb.path("repo/LICENSE-APACHE").exists(), "both, for both");
    log += &sb.devset("repo", &["add", "p/lint"]);
    assert!(sb.path("repo/lint.md").exists(), "a profile the target adds turns on what names it");
    log += &sb.devset("repo", &["apply", "--var", "license=Apache-2.0"]);
    assert!(!sb.path("repo/LICENSE-MIT").exists(), "an entry its gate turns off goes");
    log += &sb.devset("repo", &["explain", "LICENSE-MIT"]);
    sb.assert(&log, snapbox::file!["snapshots/variables_and_profiles_gate_entries.txt"]);
}

#[test]
fn existence_settles_in_one_run() {
    let sb = Sandbox::new();
    let exists = |pattern: &str| format!("when = {{ exists = [\"{pattern}\"] }}");
    let block = |pattern: &str| format!("scope = \"block\"\n{}", exists(pattern));
    sb.entries("p/agents", "agents", &[("AGENTS.md", "policy = \"once\"", "# Agents\n")]);
    sb.entries(
        "p/book",
        "book",
        &[
            ("AGENTS.md", &block("AGENTS.md"), "Build the book with `mdbook build`.\n"),
            ("c.txt", &exists("b.txt"), "c\n"),
            ("b.txt", &exists("a.txt"), "b\n"),
            ("proto.md", &exists("**/*.proto"), "protos\n"),
            ("self.txt", &exists("self.txt"), "self\n"),
        ],
    );
    sb.entries("p/base", "base", &[("a.txt", "", "a\n")]);
    let mut log = sb.devset("repo", &["init", "p/book", "--path", "../p"]);
    assert!(!sb.path("repo/c.txt").exists(), "nothing to build on yet");
    log += &sb.devset("repo", &["add", "p/agents"]);
    log += &sb.devset("repo", &["add", "p/base"]);
    assert!(sb.path("repo/c.txt").exists(), "a chain settles in one run");
    assert!(!sb.path("repo/self.txt").exists(), "an entry is never its own evidence");
    let agents = sb.read("repo/AGENTS.md");
    assert!(agents.starts_with("# Agents\n") && agents.contains("mdbook build"), "{agents}");
    sb.write("repo/api/v1.proto", "syntax = \"proto3\";\n");
    log += &sb.devset("repo", &["apply"]);
    assert!(sb.path("repo/proto.md").exists(), "a glob matches what is there");
    log += &sb.devset("repo", &["explain", "self.txt"]);
    log += &sb.devset("repo", &["status"]);
    sb.assert(&log, snapbox::file!["snapshots/existence_settles_in_one_run.txt"]);
}

#[test]
fn a_gate_turning_off_releases_its_entries() {
    let sb = Sandbox::new();
    let docs = "when = { features = [\"docs\"] }";
    sb.entries(
        "p/rust",
        "rust",
        &[("book.toml", docs, "book\n"), ("guide.md", docs, "guide\n"), ("fmt.toml", "", "fmt\n")],
    );
    sb.extend("p/rust", "[features]\ndefault = [\"docs\"]\ndocs = []\n");
    let mut log = sb.devset("repo", &["init", "p/rust", "--path", "../p"]);
    sb.write("repo/guide.md", "guide, edited\n");
    let config = sb.read("repo/.devset/config.toml");
    sb.write("repo/.devset/config.toml", &format!("{config}default-features = false\n"));
    log += &sb.devset("repo", &["status"]);
    log += &sb.devset("repo", &["apply", "--dry-run"]);
    log += &sb.devset("repo", &["apply"]);
    assert!(!sb.path("repo/book.toml").exists(), "unchanged, it goes");
    assert_eq!(sb.read("repo/guide.md"), "guide, edited\n", "edited, it stays, untracked");
    log += &sb.devset("repo", &["status", "--exit-code"]);
    sb.assert(&log, snapbox::file!["snapshots/a_gate_turning_off_releases_its_entries.txt"]);
}

#[test]
fn a_gate_on_its_own_path_keeps_what_it_adopted() {
    let sb = Sandbox::new();
    let own = "policy = \"merge\"\nwhen = { exists = [\".editorconfig\"] }";
    sb.entries("p/editor", "editor", &[(".editorconfig", own, "root = true\n")]);
    sb.write("repo/.editorconfig", "root = true\n");
    let mut log = sb.devset("repo", &["init", "--path", "../p/editor"]);
    log += &sb.devset("repo", &["apply"]);
    log += &sb.devset("repo", &["apply"]);
    assert!(sb.path("repo/.editorconfig").exists(), "a file it adopted is never its to take");
    sb.assert(&log, snapbox::file!["snapshots/a_gate_on_its_own_path_keeps_what_it_adopted.txt"]);
}

#[test]
fn gates_see_directories_and_a_profiles_own_markers() {
    let sb = Sandbox::new();
    let notes = "scope = \"block\"\ncomment = \"--\"\nwhen = { features = [\"notes\"] }";
    sb.entries(
        "p/rust",
        "rust",
        &[
            ("crates/hello/Cargo.toml", "", "[package]\nname = \"hello\"\n"),
            ("crates.md", "when = { exists = [\"crates\"] }", "Crates live in crates/.\n"),
            ("notes.xyz", notes, "a note\n"),
        ],
    );
    sb.extend("p/rust", "[features]\ndefault = [\"notes\"]\nnotes = []\n");
    sb.write("repo/notes.xyz", "mine\n");
    let mut log = sb.devset("repo", &["init", "--path", "../p/rust"]);
    assert!(sb.path("repo/crates.md").exists(), "a directory written in the run exists");
    let config = sb.read("repo/.devset/config.toml");
    sb.write("repo/.devset/config.toml", &format!("{config}default-features = false\n"));
    log += &sb.devset("repo", &["apply"]);
    assert_eq!(sb.read("repo/notes.xyz"), "mine\n", "the block goes, markers and all");
    sb.assert(
        &log,
        snapbox::file!["snapshots/gates_see_directories_and_a_profiles_own_markers.txt"],
    );
}
