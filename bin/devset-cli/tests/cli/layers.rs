//! Composing: layers side by side, the layers a profile requires, and removing one.

use core::fmt::Write as _;

use crate::sandbox::Sandbox;

#[test]
fn layers_collide_until_the_target_chooses() {
    let sb = Sandbox::new();
    sb.profile("base", "base", &[("fmt.toml", "owned", "base\n")]);
    sb.profile("rust", "rust", &[("fmt.toml", "owned", "rust\n"), ("rust.toml", "owned", "r\n")]);
    let mut log = sb.devset("repo", &["init", "--path", "../base"]);
    log += &sb.devset("repo", &["init", "--path", "../rust"]);
    let layers = "[[layers]]\npath = \"../base\"\n\n[[layers]]\npath = \"../rust\"\n";
    sb.write(
        "repo/.devset/config.toml",
        &format!("{layers}\n[files.\"fmt.toml\"]\nfrom = \"ruts\"\n"),
    );
    log += &sb.devset("repo", &["status"]);
    sb.write(
        "repo/.devset/config.toml",
        &format!("{layers}\n[files.\"fmt.toml\"]\nfrom = \"rust\"\n"),
    );
    log += &sb.devset("repo", &["apply", "--force"]);
    assert_eq!(sb.read("repo/fmt.toml"), "rust\n", "the chosen layer provides it");
    log += &sb.devset("repo", &["status", "-v"]);
    sb.assert(&log, snapbox::file!["snapshots/layers_collide_until_the_target_chooses.txt"]);
}

#[test]
fn remove_takes_a_layer_and_its_overrides() {
    let sb = Sandbox::new();
    sb.profile("base", "base", &[("a.toml", "owned", "a\n"), ("shared.toml", "owned", "1\n")]);
    sb.profile("team", "team", &[("t.toml", "owned", "t\n"), ("shared.toml", "owned", "2\n")]);
    let mut log = sb.devset("repo", &["init", "--path", "../base"]);
    let config = sb.read("repo/.devset/config.toml");
    sb.write(
        "repo/.devset/config.toml",
        &format!(
            "{config}\n[[layers]]\npath = \"../team\"\n\n# the team's copy wins\n\
             [files.\"shared.toml\"]\nfrom = \"team\"\n\n[files.\"t.toml\"]\npolicy = \"once\"\n"
        ),
    );
    log += &sb.devset("repo", &["apply"]);
    log += &sb.devset("repo", &["remove", "teem"]);
    log += &sb.devset("repo", &["remove", "team", "--dry-run"]);
    log += &sb.devset("repo", &["remove", "team"]);
    write!(log, "--- .devset/config.toml\n{}", sb.read("repo/.devset/config.toml")).unwrap();
    assert!(sb.path("repo/t.toml").exists(), "a removed layer's files stay");
    log += &sb.devset("repo", &["status", "-v"]);
    sb.assert(&log, snapbox::file!["snapshots/remove_takes_a_layer_and_its_overrides.txt"]);
}

#[test]
fn layers_stay_pinned_when_another_is_removed() {
    let sb = Sandbox::new();
    sb.publish("a", "a", &[("a.toml", "owned", "a1\n")], "a1");
    sb.publish("b", "b", &[("b.toml", "owned", "b1\n")], "b1");
    let git = ["--git", "../profiles.git", "--branch", "main"];
    sb.devset("repo", &[&["init"][..], &git, &["--path", "a"]].concat());
    sb.devset("repo", &[&["init"][..], &git, &["--path", "b"]].concat());
    sb.publish("b", "b", &[("b.toml", "owned", "b2\n")], "b2");
    let mut log = sb.devset("repo", &["remove", "a"]);
    assert_eq!(sb.read("repo/b.toml"), "b1\n", "b stays at its locked commit");
    log += &sb.devset("repo", &["update"]);
    assert_eq!(sb.read("repo/b.toml"), "b2\n", "until updated");
    sb.assert(&log, snapbox::file!["snapshots/layers_stay_pinned_when_another_is_removed.txt"]);
}

/// Adds `requires` to the `[profile]` table of the profile at `dir`.
fn requiring(sb: &Sandbox, dir: &str, requires: &str) {
    let manifest = sb.read(&format!("{dir}/profile.toml"));
    let manifest = manifest.replacen(
        "version = \"1.0.0\"\n",
        &format!("version = \"1.0.0\"\nrequires = {requires}\n"),
        1,
    );
    sb.write(&format!("{dir}/profile.toml"), &manifest);
}

#[test]
fn bundles_require_their_atoms() {
    let sb = Sandbox::new();
    sb.profile("profiles/a", "a", &[("a.toml", "owned", "a = 1\n")]);
    sb.profile("profiles/b", "b", &[("b.toml", "owned", "b = 1\n")]);
    sb.profile("profiles/both", "both", &[]);
    requiring(&sb, "profiles/both", r#"["../a", "../b"]"#);
    let mut log = sb.devset("repo", &["init", "--path", "../profiles/both"]);
    assert_eq!(
        (sb.read("repo/a.toml"), sb.read("repo/b.toml")),
        ("a = 1\n".into(), "b = 1\n".into()),
        "one layer, every atom"
    );
    log += &sb.devset("repo", &["status", "-v"]);
    write!(log, "--- .devset/config.toml\n{}", sb.read("repo/.devset/config.toml")).unwrap();
    log += &sb.devset("repo", &["update", "a"]);
    log += &sb.devset("repo", &["remove", "a"]);
    sb.assert(&log, snapbox::file!["snapshots/bundles_require_their_atoms.txt"]);
}

#[test]
fn git_siblings_share_their_requirer_commit() {
    let sb = Sandbox::new();
    sb.profile("work/profiles/a", "a", &[("a.toml", "owned", "a = 1\n")]);
    sb.profile("work/profiles/rust", "rust", &[("r.toml", "owned", "r = 1\n")]);
    requiring(&sb, "work/profiles/rust", r#"["../a"]"#);
    sb.git("work", &["init", "-q", "-b", "main"]);
    sb.git("work", &["add", "-A"]);
    sb.git("work", &["commit", "-qm", "v1"]);
    sb.git(".", &["clone", "-q", "--bare", "work", "profiles.git"]);
    let mut log = sb.devset(
        "repo",
        &["init", "--git", "../profiles.git", "--branch", "main", "--path", "profiles/rust"],
    );
    sb.write("work/profiles/a/files/a.toml", "a = 2\n");
    sb.git("work", &["commit", "-qam", "v2"]);
    sb.git("work", &["push", "-q", "../profiles.git", "main"]);
    log += &sb.devset("repo", &["apply"]);
    assert_eq!(sb.read("repo/a.toml"), "a = 1\n", "the sibling stays at the locked commit");
    log += &sb.devset("repo", &["update", "rust"]);
    assert_eq!(sb.read("repo/a.toml"), "a = 2\n", "and moves with its requirer");
    log += &sb.devset("repo", &["status"]);
    sb.assert(&log, snapbox::file!["snapshots/git_siblings_share_their_requirer_commit.txt"]);
}

#[test]
fn an_organisation_builds_on_a_collection() {
    let sb = Sandbox::new();
    sb.publish("rust", "rust", &[("rustfmt.toml", "owned", "max_width = 100\n")], "v1");
    sb.publish("rust", "rust", &[("rustfmt.toml", "owned", "max_width = 110\n")], "v2");
    sb.profile("org", "org", &[("org.toml", "owned", "org = true\n")]);
    let pin = |tag: &str| {
        format!(
            "[{{ git = \"{}\", tag = \"{tag}\", path = \"rust\" }}]",
            sb.path("profiles.git").display()
        )
    };
    requiring(&sb, "org", &pin("v1"));
    let mut log = sb.devset("repo", &["init", "--path", "../org"]);
    assert_eq!(
        sb.read("repo/rustfmt.toml"),
        "max_width = 100\n",
        "the collection at the org's tag"
    );
    log += &sb.devset("repo", &["update"]);
    assert_eq!(sb.read("repo/rustfmt.toml"), "max_width = 100\n", "a tag does not move");
    requiring(&sb, "org", "[]");
    sb.profile("org", "org", &[("org.toml", "owned", "org = true\n")]);
    requiring(&sb, "org", &pin("v2"));
    log += &sb.devset("repo", &["apply"]);
    assert_eq!(
        sb.read("repo/rustfmt.toml"),
        "max_width = 110\n",
        "until the org requires the next"
    );
    sb.assert(&log, snapbox::file!["snapshots/an_organisation_builds_on_a_collection.txt"]);
}

#[test]
fn requirements_are_refused_when_they_cannot_hold() {
    let sb = Sandbox::new();
    sb.profile("cycle/a", "a", &[]);
    requiring(&sb, "cycle/a", r#"["../b"]"#);
    sb.profile("cycle/b", "b", &[]);
    requiring(&sb, "cycle/b", r#"["../a"]"#);
    sb.profile("clash/x", "x", &[("same.toml", "owned", "x\n")]);
    sb.profile("clash/y", "y", &[("same.toml", "owned", "y\n")]);
    sb.profile("clash/both", "both", &[]);
    requiring(&sb, "clash/both", r#"["../x", "../y"]"#);
    sb.profile("bad", "bad", &[]);
    requiring(&sb, "bad", r#"[{ path = "../x" }]"#);
    let mut log = sb.devset("r1", &["init", "--path", "../cycle/a"]);
    log += &sb.devset("r2", &["init", "--path", "../clash/both"]);
    log += &sb.devset("r3", &["init", "--path", "../bad"]);
    sb.assert(&log, snapbox::file!["snapshots/requirements_are_refused_when_they_cannot_hold.txt"]);
}
