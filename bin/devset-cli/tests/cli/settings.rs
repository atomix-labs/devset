//! Settings: what the layers suggest, and what the target decides.

use crate::sandbox::Sandbox;

#[test]
fn settings_precedence() {
    let sb = Sandbox::new();
    let layers = "[sources]\np = { path = \"../p\" }\nq = { path = \"../q\" }\n\n\
                  [[layers]]\nprofile = \"p/p\"\n\n[[layers]]\nprofile = \"q/q\"\n";
    sb.profile("p", "p", &[("a.txt", "merge", "a\n")]);
    let p = sb.read("p/profile.toml");
    sb.write(
        "p/profile.toml",
        &format!("{p}\n[merge]\ndriver = \"cp %B %A\"\non-conflict = \"apply-none\"\n"),
    );
    sb.profile("q", "q", &[("b.txt", "owned", "b\n")]);
    let q = sb.read("q/profile.toml");
    sb.write("q/profile.toml", &format!("{q}\n[merge]\non-conflict = \"apply-others\"\n"));

    // A suggested driver is announced, never run.
    let mut log = sb.devset("repo", &["init", "--path", "../p"]);
    // The layers disagree and the target has not decided.
    log += &sb.devset("repo", &["add", "--path", "../q"]);
    // The target decides, adopts the driver, and overrides a layer's policy.
    sb.write(
        "repo/.devset/config.toml",
        &format!("{layers}\n[merge]\non-conflict = \"apply-none\"\ndriver = \"cp %B %A\"\n\n[files.\"b.txt\"]\npolicy = \"once\"\n"),
    );
    sb.write("repo/a.txt", "local\n");
    sb.profile("p", "p", &[("a.txt", "merge", "upstream\n")]);
    sb.write(
        "p/profile.toml",
        &format!("{p}\n[merge]\ndriver = \"cp %B %A\"\non-conflict = \"apply-none\"\n"),
    );
    log += &sb.devset("repo", &["status", "--json"]);
    log += &sb.devset("repo", &["apply"]);
    assert_eq!(
        sb.read("repo/a.txt"),
        "upstream\n",
        "the target's driver ran: `cp %B %A` takes the profile's version"
    );
    // An override of a file no layer provides.
    sb.write(
        "repo/.devset/config.toml",
        &format!("{layers}\n[files.\"gone.txt\"]\npolicy = \"once\"\n"),
    );
    log += &sb.devset("repo", &["status"]);
    sb.assert(&log, snapbox::file!["snapshots/settings_precedence.txt"]);
}
