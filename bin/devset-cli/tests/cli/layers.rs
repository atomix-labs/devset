//! Composing: layers side by side, the profiles a profile requires, and removing a layer.

use core::fmt::Write as _;

use crate::sandbox::Sandbox;

/// A target's `config.toml` naming each of `sources`, a local directory, and applying `layers`.
fn config(sources: &[(&str, &str)], layers: &[&str]) -> String {
    let mut config = String::from("[sources]\n");
    for (name, path) in sources {
        writeln!(config, "{name} = {{ path = \"{path}\" }}").unwrap();
    }
    for layer in layers {
        write!(config, "\n[[layers]]\nprofile = \"{layer}\"\n").unwrap();
    }
    config
}

#[test]
fn layers_collide_until_the_target_chooses() {
    let sb = Sandbox::new();
    sb.profile("base", "base", &[("fmt.toml", "owned", "base\n")]);
    sb.profile("rust", "rust", &[("fmt.toml", "owned", "rust\n"), ("rust.toml", "owned", "r\n")]);
    let mut log = sb.devset("repo", &["init", "--path", "../base"]);
    log += &sb.devset("repo", &["add", "--path", "../rust"]);
    let layers = config(&[("base", "../base"), ("rust", "../rust")], &["base/base", "rust/rust"]);
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
    let layers = config(&[("base", "../base"), ("team", "../team")], &["base/base", "team/team"]);
    sb.write(
        "repo/.devset/config.toml",
        &format!(
            "{layers}\n# the team's copy wins\n[files.\"shared.toml\"]\nfrom = \"team\"\n\n\
             [files.\"t.toml\"]\npolicy = \"once\"\n"
        ),
    );
    log += &sb.devset("repo", &["apply"]);
    log += &sb.devset("repo", &["remove", "teem"]);
    log += &sb.devset("repo", &["remove", "team", "--dry-run"]);
    log += &sb.devset("repo", &["remove", "team"]);
    write!(log, "--- .devset/config.toml\n{}", sb.read("repo/.devset/config.toml")).unwrap();
    assert!(sb.path("repo/t.toml").exists(), "a removed layer's `once` file stays");
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
    sb.devset("repo", &[&["add"][..], &git, &["--path", "b"]].concat());
    sb.publish("b", "b", &[("b.toml", "owned", "b2\n")], "b2");
    let mut log = sb.devset("repo", &["remove", "a"]);
    assert_eq!(sb.read("repo/b.toml"), "b1\n", "b stays at its locked commit");
    log += &sb.devset("repo", &["update"]);
    assert_eq!(sb.read("repo/b.toml"), "b2\n", "until updated");
    sb.assert(&log, snapbox::file!["snapshots/layers_stay_pinned_when_another_is_removed.txt"]);
}

#[test]
fn bundles_require_their_atoms() {
    let sb = Sandbox::new();
    sb.profile("profiles/a", "a", &[("a.toml", "owned", "a = 1\n")]);
    sb.profile("profiles/b", "b", &[("b.toml", "owned", "b = 1\n")]);
    sb.profile("profiles/both", "both", &[]);
    sb.extend("profiles/both", "[requires]\na = {}\nb = {}\n");
    let mut log = sb.devset("repo", &["init", "profiles/both", "--path", "../profiles"]);
    assert_eq!(
        (sb.read("repo/a.toml"), sb.read("repo/b.toml")),
        ("a = 1\n".into(), "b = 1\n".into()),
        "one layer, every atom"
    );
    log += &sb.devset("repo", &["status", "-v"]);
    write!(log, "--- .devset/config.toml\n{}", sb.read("repo/.devset/config.toml")).unwrap();
    log += &sb.devset("repo", &["remove", "a"]);
    log += &sb.devset("repo", &["add", "profiles/a"]);
    log += &sb.devset("repo", &["remove", "a"]);
    assert!(sb.path("repo/a.toml").exists(), "a layer another requires stays");
    sb.assert(&log, snapbox::file!["snapshots/bundles_require_their_atoms.txt"]);
}

#[test]
fn a_source_moves_as_one() {
    let sb = Sandbox::new();
    sb.profile("work/profiles/a", "a", &[("a.toml", "owned", "a = 1\n")]);
    sb.profile("work/profiles/rust", "rust", &[("r.toml", "owned", "r = 1\n")]);
    sb.extend("work/profiles/rust", "[requires]\na = {}\n");
    sb.git("work", &["init", "-q", "-b", "main"]);
    sb.git("work", &["add", "-A"]);
    sb.git("work", &["commit", "-qm", "v1"]);
    sb.git(".", &["clone", "-q", "--bare", "work", "profiles.git"]);
    let mut log =
        sb.devset("repo", &["init", "acme/rust", "--git", "../profiles.git", "--branch", "main"]);
    sb.write("work/profiles/a/files/a.toml", "a = 2\n");
    sb.git("work", &["commit", "-qam", "v2"]);
    sb.git("work", &["push", "-q", "../profiles.git", "main"]);
    log += &sb.devset("repo", &["apply"]);
    assert_eq!(sb.read("repo/a.toml"), "a = 1\n", "the source stays at the locked commit");
    log += &sb.devset("repo", &["update", "rust"]);
    assert_eq!(sb.read("repo/a.toml"), "a = 2\n", "and moves as one, by a layer's name");
    log += &sb.devset("repo", &["status"]);
    sb.assert(&log, snapbox::file!["snapshots/a_source_moves_as_one.txt"]);
}

#[test]
fn an_organisation_builds_on_a_collection() {
    let sb = Sandbox::new();
    sb.publish("rust", "rust", &[("rustfmt.toml", "owned", "max_width = 100\n")], "v1");
    sb.publish("rust", "rust", &[("rustfmt.toml", "owned", "max_width = 110\n")], "v2");
    let git = sb.path("profiles.git");
    let requires = |tag: &str| {
        format!("[requires]\nrust = {{ git = \"{}\", tag = \"{tag}\" }}\n", git.display())
    };
    sb.profile("org", "org", &[("org.toml", "owned", "org = true\n")]);
    sb.extend("org", &requires("v1"));
    let mut log = sb.devset("repo", &["init", "--path", "../org"]);
    assert_eq!(
        sb.read("repo/rustfmt.toml"),
        "max_width = 100\n",
        "the collection at the org's tag"
    );
    log += &sb.devset("repo", &["update"]);
    assert_eq!(sb.read("repo/rustfmt.toml"), "max_width = 100\n", "a tag does not move");
    sb.profile("org", "org", &[("org.toml", "owned", "org = true\n")]);
    sb.extend("org", &requires("v2"));
    log += &sb.devset("repo", &["apply"]);
    assert_eq!(
        sb.read("repo/rustfmt.toml"),
        "max_width = 110\n",
        "until the org requires the next"
    );
    log += &sb.devset("repo", &["status"]);
    sb.assert(&log, snapbox::file!["snapshots/an_organisation_builds_on_a_collection.txt"]);
}

#[test]
fn requirements_are_refused_when_they_cannot_hold() {
    let sb = Sandbox::new();
    sb.profile("cycle/a", "a", &[]);
    sb.extend("cycle/a", "[requires]\nb = {}\n");
    sb.profile("cycle/b", "b", &[]);
    sb.extend("cycle/b", "[requires]\na = {}\n");
    sb.profile("clash/x", "x", &[("same.toml", "owned", "x\n")]);
    sb.profile("clash/y", "y", &[("same.toml", "owned", "y\n")]);
    sb.profile("clash/both", "both", &[]);
    sb.extend("clash/both", "[requires]\nx = {}\ny = {}\n");
    sb.profile("bad", "bad", &[]);
    sb.extend("bad", "[requires]\nx = { path = \"../x\" }\n");
    sb.profile("lost", "lost", &[]);
    sb.extend("lost", "[requires]\nnowhere = {}\n");
    let mut log = sb.devset("r1", &["init", "cycle/a", "--path", "../cycle"]);
    log += &sb.devset("r2", &["init", "clash/both", "--path", "../clash"]);
    log += &sb.devset("r3", &["init", "--path", "../bad"]);
    log += &sb.devset("r4", &["init", "--path", "../lost"]);
    sb.assert(&log, snapbox::file!["snapshots/requirements_are_refused_when_they_cannot_hold.txt"]);
}
