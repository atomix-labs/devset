//! Features: unified across requirers, turned on by default or by name, and explained.

use core::fmt::Write as _;

use crate::sandbox::Sandbox;

/// A source `p` of three profiles: `ci`, `book`, which can turn on `ci`'s pages, and `rust`,
/// which requires `book` and turns its pages on with `docs`.
fn source(sb: &Sandbox) {
    let gated = |feature: &str| format!("when = {{ features = [\"{feature}\"] }}");
    let (pages, nightly) = (gated("pages"), gated("nightly"));
    sb.entries(
        "p/ci",
        "ci",
        &[
            ("ci.yml", "", "ci\n"),
            ("pages.yml", &pages, "pages\n"),
            ("nightly.yml", &nightly, "nightly\n"),
        ],
    );
    sb.extend("p/ci", "[features]\npages = []\nnightly = []\n");
    let (katex, api) = (gated("katex"), gated("api"));
    sb.entries(
        "p/book",
        "book",
        &[("book.toml", "", "book\n"), ("katex.css", &katex, "katex\n"), ("api.js", &api, "api\n")],
    );
    sb.extend(
        "p/book",
        "[requires]\nci = { optional = true }\nlychee = { optional = true }\n\n\
         [features]\ndefault = [\"katex\"]\nkatex = []\napi = [\"katex\"]\npages = [\"ci/pages\"]\nlinks = [\"dep:lychee\"]\n",
    );
    sb.entries("p/lychee", "lychee", &[("lychee.toml", "", "lychee\n")]);
    sb.entries("p/rust", "rust", &[("rustfmt.toml", "", "rust\n")]);
    sb.extend(
        "p/rust",
        "[requires]\nbook = { features = [\"api\"] }\n\n[features]\ndocs = [\"book/pages\"]\n",
    );
}

#[test]
fn features_unify_across_requirers() {
    let sb = Sandbox::new();
    source(&sb);
    let mut log = sb.devset("repo", &["init", "p/rust", "--path", "../p", "--features", "docs"]);
    for (file, there) in [
        ("katex.css", true),
        ("api.js", true),
        ("pages.yml", true),
        ("nightly.yml", false),
        ("lychee.toml", false),
    ] {
        assert_eq!(sb.path(&format!("repo/{file}")).exists(), there, "{file}");
    }
    log += &sb.devset("repo", &["features"]);
    log += &sb.devset("repo", &["features", "book"]);
    log += &sb.devset("repo", &["status", "-v"]);
    write!(log, "--- .devset/lock.toml\n{}", sb.read("repo/.devset/lock.toml")).unwrap();
    sb.assert(&log, snapbox::file!["snapshots/features_unify_across_requirers.txt"]);
}

#[test]
fn a_feature_turned_off_releases_what_it_added() {
    let sb = Sandbox::new();
    source(&sb);
    let mut log = sb.devset("repo", &["init", "p/rust", "--path", "../p", "--features", "docs"]);
    sb.write("repo/pages.yml", "pages, edited\n");
    let config = sb.read("repo/.devset/config.toml").replace("features = [\"docs\"]\n", "");
    sb.write("repo/.devset/config.toml", &config);
    log += &sb.devset("repo", &["status"]);
    log += &sb.devset("repo", &["apply"]);
    assert_eq!(sb.read("repo/pages.yml"), "pages, edited\n", "an edited file stays, untracked");
    log += &sb.devset("repo", &["add", "p/book", "--no-default-features", "--features", "links"]);
    assert!(sb.path("repo/lychee.toml").exists(), "`dep:` activates an optional requirement");
    log += &sb.devset("repo", &["features", "book"]);
    log += &sb.devset("repo", &["add", "p/ci", "--features", "nihgtly"]);
    sb.assert(&log, snapbox::file!["snapshots/a_feature_turned_off_releases_what_it_added.txt"]);
}
