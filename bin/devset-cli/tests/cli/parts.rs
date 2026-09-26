//! Parts: the keys, or the block, a profile owns in a file the target owns.

use core::fmt::Write as _;

use crate::sandbox::Sandbox;

#[test]
fn keys_join_the_target_file() {
    let sb = Sandbox::new();
    let lints = "[workspace.lints.clippy]\nunwrap_used = \"deny\"\npedantic = { level = \"deny\", priority = -1 }\n";
    sb.entries("lints", "lints", &[("Cargo.toml", "scope = \"keys\"", lints)]);
    sb.write(
        "repo/Cargo.toml",
        "[workspace]\nmembers = [\"a\"]\n\n[workspace.lints.clippy]\n# ours\nmodule_name_repetitions = \"allow\"\nunwrap_used = \"warn\"\n",
    );
    let mut log = sb.devset("repo", &["init", "--path", "../lints"]);
    let cargo = sb.read("repo/Cargo.toml");
    assert!(cargo.contains("unwrap_used = \"warn\""), "adopted: a key the file holds stays");
    assert!(cargo.contains("pedantic"), "and one it lacks is written");
    write!(log, "--- Cargo.toml\n{cargo}").unwrap();
    log += &sb.devset("repo", &["status", "--exit-code"]);
    log += &sb.devset("repo", &["diff"]);
    log += &sb.devset("repo", &["apply", "--force"]);
    let cargo = sb.read("repo/Cargo.toml");
    assert!(cargo.contains("unwrap_used = \"deny\""), "--force restores the shared key");
    assert!(cargo.contains("# ours\nmodule_name_repetitions = \"allow\""), "and no other");
    write!(log, "--- Cargo.toml\n{cargo}").unwrap();

    let cargo = cargo
        .replace("module_name_repetitions = \"allow\"", "module_name_repetitions = \"warn\"")
        .replace("unwrap_used = \"deny\"", "unwrap_used   =   'deny'   # why");
    sb.write("repo/Cargo.toml", &format!("{cargo}too_many_lines = \"allow\"\n"));
    log += &sb.devset("repo", &["status", "--exit-code"]);
    write!(log, "--- .devset/state.toml\n{}", sb.read("repo/.devset/state.toml")).unwrap();

    sb.entries("lints", "lints", &[]);
    log += &sb.devset("repo", &["apply"]);
    let cargo = sb.read("repo/Cargo.toml");
    assert!(!cargo.contains("pedantic"), "a dropped part as recorded goes, reformatted or not");
    assert!(cargo.contains("too_many_lines"), "and the file's own keys stay");
    sb.assert(&log, snapbox::file!["snapshots/keys_join_the_target_file.txt"]);
}

#[test]
fn a_key_that_changes_hands_stays() {
    let sb = Sandbox::new();
    let keys = "scope = \"keys\"";
    let lints = "[workspace.lints.rust]\nunsafe_code = \"forbid\"\n";
    sb.entries("old", "old", &[("Cargo.toml", keys, lints)]);
    sb.entries("new", "new", &[("new.toml", "", "n = 1\n")]);
    sb.write("repo/Cargo.toml", "[workspace]\nmembers = [\"a\"]\n");
    let mut log = sb.devset("repo", &["init", "--path", "../old"]);
    log += &sb.devset("repo", &["init", "--path", "../new"]);
    sb.entries("old", "old", &[]);
    sb.entries("new", "new", &[("new.toml", "", "n = 1\n"), ("Cargo.toml", keys, lints)]);
    log += &sb.devset("repo", &["apply"]);
    let cargo = sb.read("repo/Cargo.toml");
    assert!(cargo.contains("unsafe_code = \"forbid\""), "the key the new owner took stays");
    write!(log, "--- Cargo.toml\n{cargo}").unwrap();
    log += &sb.devset("repo", &["status", "--exit-code"]);
    sb.assert(&log, snapbox::file!["snapshots/a_key_that_changes_hands_stays.txt"]);
}

#[test]
fn a_dropped_profile_takes_back_what_is_still_its_own() {
    let sb = Sandbox::new();
    let (block, keys, once) = ("scope = \"block\"", "scope = \"keys\"", "policy = \"once\"");
    sb.entries(
        "gone",
        "gone",
        &[
            ("as-written.toml", "", "a = 1\n"),
            ("edited.toml", "", "b = 1\n"),
            ("once.toml", once, "c = 1\n"),
            ("nested/deep/d.toml", "", "d = 1\n"),
            ("justfile", block, "import? '.just/gone.just'\n"),
            (".gitignore", block, "/target\n"),
            ("Cargo.toml", keys, "[workspace.lints.rust]\nunsafe_code = \"forbid\"\n"),
        ],
    );
    sb.write("repo/justfile", "default:\n    @just --list\n");
    sb.write("repo/Cargo.toml", "[workspace]\nmembers = [\"a\"]\n");
    let mut log = sb.devset("repo", &["init", "--path", "../gone"]);
    write!(log, "--- .devset/state.toml\n{}", sb.read("repo/.devset/state.toml")).unwrap();
    sb.write("repo/edited.toml", "b = 2\n");
    sb.entries("gone", "gone", &[]);
    log += &sb.devset("repo", &["status"]);
    log += &sb.devset("repo", &["apply"]);
    assert!(!sb.path("repo/as-written.toml").exists(), "a file as devset wrote it goes");
    assert!(!sb.path("repo/nested").exists(), "with the directories it leaves empty");
    assert_eq!(sb.read("repo/edited.toml"), "b = 2\n", "an edited file stays");
    assert_eq!(sb.read("repo/once.toml"), "c = 1\n", "as does one that became the target's");
    assert_eq!(
        sb.read("repo/justfile"),
        "default:\n    @just --list\n",
        "a block goes, and the rest of its file stays as it was"
    );
    assert!(!sb.path("repo/.gitignore").exists(), "a file that was only the block goes");
    let cargo = sb.read("repo/Cargo.toml");
    assert!(!cargo.contains("unsafe_code"), "keys go");
    assert!(cargo.contains("members"), "and the file's own stay");
    write!(log, "--- Cargo.toml\n{cargo}").unwrap();
    log += &sb.devset("repo", &["status", "--exit-code"]);
    sb.assert(
        &log,
        snapbox::file!["snapshots/a_dropped_profile_takes_back_what_is_still_its_own.txt"],
    );
}

/// `text` with each conflict resolved to the profile's side, its markers gone.
fn theirs(text: &str) -> String {
    let mut side = "";
    let mut out = String::new();
    for line in text.split_inclusive('\n') {
        match line.get(..7).unwrap_or_default() {
            "<<<<<<<" | "|||||||" => side = "skip",
            "=======" if !side.is_empty() => side = "keep",
            ">>>>>>>" => side = "",
            _ if side == "skip" => {},
            _ => out.push_str(line),
        }
    }
    out
}

#[test]
fn keys_merge_leaf_by_leaf() {
    let sb = Sandbox::new();
    let entry = "scope = \"keys\"\npolicy = \"merge\"";
    let v1 = "[bans]\nmultiple-versions = \"deny\"\nwildcards = \"deny\"\n\n[licenses]\nallow = [\"MIT\"]\nconfidence-threshold = 0.9\n";
    sb.entries("work/p", "p", &[("deny.toml", entry, v1)]);
    sb.release("v1");
    let mut log =
        sb.devset("repo", &["init", "--git", "../profiles.git", "--branch", "main", "--path", "p"]);
    let edited = sb
        .read("repo/deny.toml")
        .replace("wildcards = \"deny\"", "wildcards = \"allow\" # a local exception");
    sb.write(
        "repo/deny.toml",
        &format!("[graph]\ntargets = [\"x86_64-unknown-linux-gnu\"]\n\n{edited}"),
    );
    let v2 = "[bans]\nmultiple-versions = \"deny\"\nwildcards = \"deny\"\nhighlight = \"all\"\n\n[licenses]\nallow = [\"MIT\", \"Apache-2.0\"]\n";
    sb.entries("work/p", "p", &[("deny.toml", entry, v2)]);
    sb.release("v2");
    log += &sb.devset("repo", &["update"]);
    let deny = sb.read("repo/deny.toml");
    assert!(deny.contains("wildcards = \"allow\" # a local exception"), "ours kept");
    assert!(deny.contains("allow = [\"MIT\", \"Apache-2.0\"]"), "theirs taken");
    assert!(!deny.contains("confidence-threshold"), "a key the profile dropped goes");
    assert!(deny.contains("[graph]"), "the target's own tables stay");
    write!(log, "--- deny.toml\n{deny}").unwrap();

    let v3 = v2.replace("wildcards = \"deny\"", "wildcards = \"warn\"");
    sb.entries("work/p", "p", &[("deny.toml", entry, &v3)]);
    sb.release("v3");
    log += &sb.devset("repo", &["update"]);
    assert_eq!(sb.read("repo/deny.toml"), deny, "a conflict leaves the file");
    let sidecar = sb.read("repo/.devset/conflicts/deny.toml");
    write!(log, "--- .devset/conflicts/deny.toml\n{sidecar}").unwrap();
    log += &sb.devset("repo", &["status"]);
    sb.write("repo/.devset/conflicts/deny.toml", &theirs(&sidecar));
    log += &sb.devset("repo", &["update", "--continue"]);
    assert!(sb.read("repo/deny.toml").contains("wildcards = \"warn\""), "resolved");
    log += &sb.devset("repo", &["status", "--exit-code"]);
    sb.assert(&log, snapbox::file!["snapshots/keys_merge_leaf_by_leaf.txt"]);
}

#[test]
fn layers_share_a_file_in_parts() {
    let sb = Sandbox::new();
    let keys = "scope = \"keys\"";
    sb.entries(
        "lints",
        "lints",
        &[("Cargo.toml", keys, "[workspace.lints.clippy]\nunwrap_used = \"deny\"\n")],
    );
    sb.entries(
        "rust",
        "rust",
        &[("Cargo.toml", keys, "[workspace.package]\nedition = \"2024\"\n\n[workspace.lints.rust]\nunsafe_code = \"forbid\"\n")],
    );
    sb.entries(
        "strict",
        "strict",
        &[("Cargo.toml", keys, "[workspace.lints]\nclippy = { all = \"deny\" }\n")],
    );
    sb.profile("whole", "whole", &[("Cargo.toml", "owned", "[package]\nname = \"x\"\n")]);
    sb.write("repo/Cargo.toml", "[workspace]\nmembers = []\n");
    let mut log = sb.devset("repo", &["init", "--path", "../lints"]);
    log += &sb.devset("repo", &["add", "--path", "../rust"]);
    write!(log, "--- Cargo.toml\n{}", sb.read("repo/Cargo.toml")).unwrap();
    log += &sb.devset("repo", &["status", "-v"]);
    log += &sb.devset("repo", &["add", "--path", "../strict"]);
    log += &sb.devset("repo", &["add", "--path", "../whole"]);
    let names = ["lints", "rust", "whole"];
    let mut config = String::from("[sources]\n");
    for name in names {
        writeln!(config, "{name} = {{ path = \"../{name}\" }}").unwrap();
    }
    for name in names {
        write!(config, "\n[[layers]]\nprofile = \"{name}/{name}\"\n").unwrap();
    }
    config += "\n[files.\"Cargo.toml\"]\nfrom = \"lints\"\n";
    sb.write("repo/.devset/config.toml", &config);
    log += &sb.devset("repo", &["apply"]);
    assert!(!sb.read("repo/Cargo.toml").contains("[package]"), "`from` picks the part");
    log += &sb.devset("repo", &["status"]);
    sb.assert(&log, snapbox::file!["snapshots/layers_share_a_file_in_parts.txt"]);
}

#[test]
fn json_and_yaml_keys_keep_comments() {
    let sb = Sandbox::new();
    let keys = "scope = \"keys\"";
    sb.entries(
        "editor",
        "editor",
        &[
            (
                ".vscode/settings.json",
                keys,
                "{\n  \"editor.formatOnSave\": true,\n  \"[toml]\": { \"editor.tabSize\": 2 }\n}\n",
            ),
            (
                ".github/dependabot.yml",
                keys,
                "version: 2\nupdates:\n  - package-ecosystem: cargo\n    directory: /\n    schedule:\n      interval: weekly\n",
            ),
        ],
    );
    sb.write(
        "repo/.vscode/settings.json",
        "{\n  // ours\n  \"rust-analyzer.cargo.target\": \"aarch64-unknown-linux-gnu\",\n  \"editor.formatOnSave\": false,\n}\n",
    );
    sb.write(
        "repo/.github/dependabot.yml",
        "# ours\nversion: 2\nregistries:\n  crates:\n    type: cargo-registry\n    url: https://example.com\n",
    );
    let mut log = sb.devset("repo", &["init", "--path", "../editor"]);
    for file in [".vscode/settings.json", ".github/dependabot.yml"] {
        write!(log, "--- {file}\n{}", sb.read(&format!("repo/{file}"))).unwrap();
    }
    log += &sb.devset("repo", &["apply", "--force"]);
    let settings = sb.read("repo/.vscode/settings.json");
    assert!(settings.contains("\"editor.formatOnSave\": true"), "restored");
    assert!(settings.contains("// ours"), "comments survive");
    write!(log, "--- .vscode/settings.json\n{settings}").unwrap();
    log += &sb.devset("repo", &["status", "--exit-code"]);
    sb.assert(&log, snapbox::file!["snapshots/json_and_yaml_keys_keep_comments.txt"]);
}

#[test]
fn blocks_are_marked_and_stay_where_moved() {
    let sb = Sandbox::new();
    let block = "scope = \"block\"";
    sb.entries("rust", "rust", &[(".gitignore", block, "/target\n")]);
    sb.entries("node", "node", &[(".gitignore", block, "node_modules/\n")]);
    sb.write("repo/.gitignore", "/deployments/local/*\n");
    let mut log = sb.devset("repo", &["init", "--path", "../rust"]);
    log += &sb.devset("repo", &["init", "--path", "../node"]);
    write!(log, "--- .gitignore\n{}", sb.read("repo/.gitignore")).unwrap();
    let moved = "# >>> devset: rust >>>\n/target\n# <<< devset: rust <<<\n/deployments/local/*\n.env\n\n# >>> devset: node >>>\nnode_modules/\n# <<< devset: node <<<\n";
    sb.write("repo/.gitignore", moved);
    log += &sb.devset("repo", &["status", "--exit-code"]);
    sb.entries("rust", "rust", &[(".gitignore", block, "/target\n**/*.rs.bk\n")]);
    log += &sb.devset("repo", &["apply"]);
    assert_eq!(
        sb.read("repo/.gitignore"),
        moved.replace("/target\n", "/target\n**/*.rs.bk\n"),
        "a moved block is updated where it is"
    );
    sb.write("repo/.gitignore", &moved.replace("# <<< devset: node <<<\n", ""));
    log += &sb.devset("repo", &["status"]);
    sb.assert(&log, snapbox::file!["snapshots/blocks_are_marked_and_stay_where_moved.txt"]);
}

#[test]
fn parts_keep_their_file_valid() {
    let sb = Sandbox::new();
    sb.entries("p", "p", &[("Cargo.toml", "scope = \"block\"", "[workspace]\nresolver = \"3\"\n")]);
    sb.entries("keys", "keys", &[("notes.txt", "scope = \"keys\"", "a = 1\n")]);
    sb.entries("json", "json", &[("package.json", "scope = \"block\"", "\"x\": 1\n")]);
    sb.write("repo/Cargo.toml", "[workspace]\nmembers = []\n");
    let mut log = sb.devset("repo", &["init", "--path", "../p"]);
    assert_eq!(sb.read("repo/Cargo.toml"), "[workspace]\nmembers = []\n", "left as it was");
    write!(
        log,
        "--- .devset/conflicts/Cargo.toml\n{}",
        sb.read("repo/.devset/conflicts/Cargo.toml")
    )
    .unwrap();
    log += &sb.devset("r2", &["init", "--path", "../keys"]);
    log += &sb.devset("r3", &["init", "--path", "../json"]);
    sb.write("r4/Cargo.toml", "[workspace\n");
    log += &sb.devset("r4", &["init", "--path", "../p"]);
    sb.assert(&log, snapbox::file!["snapshots/parts_keep_their_file_valid.txt"]);
}
