//! End to end: the `devset` binary against real directories and git repositories.

#[cfg(test)]
mod tests {
    use core::fmt::Write as _;
    use std::ffi::OsStr;
    use std::os::unix::fs::symlink;
    use std::path::PathBuf;
    use std::process::Command;
    use std::{env, fs};

    use snapbox::cmd::cargo_bin;
    use snapbox::{Assert, Data, Redactions};
    use tempfile::TempDir;

    /// A hermetic sandbox with its own `HOME`, cache and git configuration.
    struct Sandbox {
        /// Removed on drop.
        _dir: TempDir,
        /// `dir`, canonical, as devset sees it: under `/private/var` on macOS.
        root: PathBuf,
    }

    impl Sandbox {
        /// An empty sandbox.
        fn new() -> Self {
            let dir = TempDir::new().unwrap();
            let root = dir.path().canonicalize().unwrap();
            Self { _dir: dir, root }
        }

        /// Absolute path of `rel`.
        fn path(&self, rel: &str) -> PathBuf {
            self.root.join(rel)
        }

        /// Writes `text` to `rel`, creating parent directories.
        fn write(&self, rel: &str, text: &str) {
            let path = self.path(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }

        /// Contents of `rel`.
        fn read(&self, rel: &str) -> String {
            fs::read_to_string(self.path(rel)).unwrap()
        }

        /// Writes a profile at `dir`: `(path, policy, content)` per file.
        fn profile(&self, dir: &str, name: &str, files: &[(&str, &str, &str)]) {
            let mut manifest = format!("[profile]\nname = \"{name}\"\nversion = \"1.0.0\"\n");
            for (path, policy, content) in files {
                write!(manifest, "\n[files.\"{path}\"]\npolicy = \"{policy}\"\n").unwrap();
                self.write(&format!("{dir}/files/{path}"), content);
            }
            self.write(&format!("{dir}/profile.toml"), &manifest);
        }

        /// Runs `program` in `cwd` with a cleared environment.
        fn command(&self, program: impl AsRef<OsStr>, cwd: &str, args: &[&str]) -> Command {
            fs::create_dir_all(self.path(cwd)).unwrap();
            let mut command = Command::new(program);
            command
                .args(args)
                .current_dir(self.path(cwd))
                .env_clear()
                .env("PATH", env::var_os("PATH").unwrap_or_default())
                .env("HOME", self.path("home"))
                .env("XDG_CACHE_HOME", self.path("cache"))
                .env("NO_COLOR", "1")
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_CONFIG_SYSTEM", "/dev/null")
                .env("GIT_AUTHOR_NAME", "test")
                .env("GIT_AUTHOR_EMAIL", "test@example.com")
                .env("GIT_AUTHOR_DATE", "2026-01-01T00:00:00Z")
                .env("GIT_COMMITTER_NAME", "test")
                .env("GIT_COMMITTER_EMAIL", "test@example.com")
                .env("GIT_COMMITTER_DATE", "2026-01-01T00:00:00Z");
            command
        }

        /// Runs `devset` in `cwd`, logging the command line, its output and its exit code.
        fn devset(&self, cwd: &str, args: &[&str]) -> String {
            let out = self.command(env!("CARGO_BIN_EXE_devset"), cwd, args).output().unwrap();
            let code = out.status.code().unwrap();
            let text = format!(
                "$ devset {}\n{}{}[exit {code}]\n",
                args.join(" "),
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr),
            );
            text
        }

        /// Checks `log` against the snapshot `expected`, the sandbox's path as `[ROOT]`.
        ///
        /// `SNAPSHOTS=overwrite` updates the snapshot instead.
        fn assert(&self, log: &str, expected: Data) {
            let mut redactions = Redactions::new();
            redactions.insert("[ROOT]", self.root.clone()).unwrap();
            Assert::new()
                .action_env("SNAPSHOTS")
                .redact_with(redactions)
                .eq(log.to_owned(), expected);
        }

        /// [`Sandbox::devset`] with `vars` added to the environment.
        fn devset_with(&self, vars: &[(&str, &str)], cwd: &str, args: &[&str]) -> String {
            let mut command = self.command(cargo_bin!("devset"), cwd, args);
            command.envs(vars.iter().copied());
            let out = command.output().unwrap();
            let code = out.status.code().unwrap();
            format!(
                "$ devset {}\n{}{}[exit {code}]\n",
                args.join(" "),
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr),
            )
        }

        /// Runs `git` in `cwd`, which must succeed.
        fn git(&self, cwd: &str, args: &[&str]) -> String {
            let out = self.command("git", cwd, args).output().unwrap();
            assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
            String::from_utf8_lossy(&out.stdout).trim().to_owned()
        }

        /// Publishes `work/<dir>` as a tagged commit of the bare repository `profiles.git`.
        fn publish(&self, dir: &str, name: &str, files: &[(&str, &str, &str)], tag: &str) {
            self.profile(&format!("work/{dir}"), name, files);
            if !self.path("work/.git").exists() {
                self.git("work", &["init", "-q", "-b", "main"]);
            }
            self.git("work", &["add", "-A"]);
            self.git("work", &["commit", "-qm", tag]);
            self.git("work", &["tag", "-a", tag, "-m", tag]);
            if self.path("profiles.git").exists() {
                self.git("work", &["push", "-q", "../profiles.git", "main", tag]);
            } else {
                self.git(".", &["clone", "-q", "--bare", "work", "profiles.git"]);
            }
        }

        /// Number of blobs in `target`'s base store.
        fn blobs(&self, target: &str) -> usize {
            fs::read_dir(self.path(&format!("{target}/.devset/base"))).unwrap().count()
        }
    }

    #[test]
    fn adoption_and_the_drift_gate() {
        let sb = Sandbox::new();
        sb.profile(
            "profile",
            "demo",
            &[
                ("rustfmt.toml", "owned", "max_width = 100\n"),
                ("deny.toml", "merge", "[bans]\n"),
                ("justfile", "once", "default:\n"),
            ],
        );
        sb.write("repo/deny.toml", "[bans]\ndeny = [\"openssl\"]\n");
        let mut log = sb.devset("repo", &["init", "--path", "../profile"]);
        assert_eq!(
            sb.read("repo/deny.toml"),
            "[bans]\ndeny = [\"openssl\"]\n",
            "adoption keeps the file"
        );
        log += &sb.devset("repo", &["status", "--exit-code"]);

        sb.write("repo/rustfmt.toml", "max_width = 80\n");
        log += &sb.devset("repo", &["status", "--exit-code"]);
        log += &sb.devset("repo", &["apply"]);
        assert_eq!(
            sb.read("repo/rustfmt.toml"),
            "max_width = 80\n",
            "plain apply never destroys an edit"
        );
        log += &sb.devset("repo", &["apply", "--force", "--dry-run"]);
        log += &sb.devset("repo", &["apply", "--force"]);
        assert_eq!(
            sb.read("repo/rustfmt.toml"),
            "max_width = 100\n",
            "--force restores owned files"
        );
        log += &sb.devset("repo", &["status", "--exit-code"]);
        sb.assert(&log, snapbox::file!["snapshots/adoption_and_the_drift_gate.txt"]);
    }

    #[test]
    fn cosmetic_edits_are_not_drift() {
        let sb = Sandbox::new();
        sb.profile("profile", "demo", &[("a.toml", "owned", "a = 1\n")]);
        let mut log = sb.devset("repo", &["init", "--path", "../profile"]);
        sb.write("repo/a.toml", "\u{FEFF}a = 1  \r\n\n\n");
        log += &sb.devset("repo", &["status", "--exit-code"]);
        log += &sb.devset("repo", &["apply", "--force"]);
        assert_eq!(
            sb.read("repo/a.toml"),
            "\u{FEFF}a = 1  \r\n\n\n",
            "a reformatted file is left alone"
        );
        sb.assert(&log, snapbox::file!["snapshots/cosmetic_edits_are_not_drift.txt"]);
    }

    #[test]
    fn profile_changes_update_and_release() {
        let sb = Sandbox::new();
        sb.profile(
            "profile",
            "demo",
            &[("a.toml", "owned", "a = 1\n"), ("b.toml", "owned", "b = 1\n")],
        );
        let mut log = sb.devset("repo", &["init", "--path", "../profile"]);
        assert_eq!(sb.blobs("repo"), 2, "one blob per base");

        sb.profile("profile", "demo", &[("a.toml", "owned", "a = 2\n")]);
        log += &sb.devset("repo", &["status"]);
        log += &sb.devset("repo", &["apply"]);
        log += &sb.devset("repo", &["apply"]);
        assert_eq!(sb.read("repo/a.toml"), "a = 2\n", "update");
        assert_eq!(sb.read("repo/b.toml"), "b = 1\n", "a released file stays on disk");
        assert_eq!(sb.blobs("repo"), 1, "unreferenced blobs are collected");
        sb.assert(&log, snapbox::file!["snapshots/profile_changes_update_and_release.txt"]);
    }

    #[test]
    fn once_is_written_once() {
        let sb = Sandbox::new();
        sb.profile("profile", "demo", &[("justfile", "once", "v1\n")]);
        let mut log = sb.devset("repo", &["init", "--path", "../profile"]);
        sb.profile("profile", "demo", &[("justfile", "once", "v2\n")]);
        log += &sb.devset("repo", &["apply", "--force"]);
        assert_eq!(sb.read("repo/justfile"), "v1\n", "never rewritten");
        fs::remove_file(sb.path("repo/justfile")).unwrap();
        log += &sb.devset("repo", &["apply", "--force"]);
        assert!(!sb.path("repo/justfile").exists(), "a deletion is respected");
        sb.assert(&log, snapbox::file!["snapshots/once_is_written_once.txt"]);
    }

    #[test]
    fn git_sources_are_pinned_by_the_lock() {
        let sb = Sandbox::new();
        sb.profile("work/rust", "rust", &[("rustfmt.toml", "owned", "edition = \"2024\"\n")]);
        sb.git("work", &["init", "-q", "-b", "main"]);
        sb.git("work", &["add", "-A"]);
        sb.git("work", &["commit", "-qm", "v1"]);
        sb.git("work", &["tag", "-a", "v1", "-m", "v1"]);
        sb.git(".", &["clone", "-q", "--bare", "work", "profiles.git"]);

        let mut log = sb
            .devset("repo", &["init", "--git", "../profiles.git", "--tag", "v1", "--path", "rust"]);
        log += &sb.devset("repo", &["status"]);
        write!(log, "--- lock.toml\n{}", sb.read("repo/.devset/lock.toml")).unwrap();

        // The tag moves upstream; the lock still names the original commit, on any machine.
        sb.profile("work/rust", "rust", &[("rustfmt.toml", "owned", "edition = \"2027\"\n")]);
        sb.git("work", &["commit", "-qam", "v2"]);
        sb.git("work", &["tag", "-fa", "v1", "-m", "moved"]);
        sb.git("work", &["push", "-qf", "../profiles.git", "main", "v1"]);
        fs::remove_dir_all(sb.path("cache")).unwrap();
        fs::remove_file(sb.path("repo/rustfmt.toml")).unwrap();
        log += &sb.devset("repo", &["apply", "--force"]);
        assert_eq!(
            sb.read("repo/rustfmt.toml"),
            "edition = \"2024\"\n",
            "the locked commit, not the moved tag"
        );
        sb.assert(&log, snapbox::file!["snapshots/git_sources_are_pinned_by_the_lock.txt"]);
    }

    #[test]
    fn worktrees_and_subdirectories_find_their_target() {
        let sb = Sandbox::new();
        sb.profile("profile", "demo", &[("a.toml", "owned", "a = 1\n")]);
        sb.git("repo", &["init", "-q", "-b", "main"]);
        let mut log = sb.devset("repo", &["init", "--path", "../profile"]);
        sb.git("repo", &["add", "-A"]);
        sb.git("repo", &["commit", "-qm", "adopt devset"]);
        sb.git("repo", &["worktree", "add", "-q", "../wt"]);
        log += &sb.devset("wt/.devset", &["status", "--exit-code"]);
        sb.assert(
            &log,
            snapbox::file!["snapshots/worktrees_and_subdirectories_find_their_target.txt"],
        );
    }

    #[test]
    fn dry_run_writes_nothing() {
        let sb = Sandbox::new();
        sb.profile("profile", "demo", &[("a.toml", "owned", "a = 1\n")]);
        let log = sb.devset("repo", &["init", "--path", "../profile", "--dry-run"]);
        assert!(!sb.path("repo/.devset").exists(), "no state");
        assert!(!sb.path("repo/a.toml").exists(), "no files");
        sb.assert(&log, snapbox::file!["snapshots/dry_run_writes_nothing.txt"]);
    }

    #[test]
    fn state_lives_in_dot_devset() {
        let sb = Sandbox::new();
        sb.profile("base", "base", &[("a.toml", "owned", "a = 1\n")]);
        sb.profile("extra", "extra", &[("b.toml", "owned", "b = 1\n")]);
        let mut log = sb.devset("repo", &["init", "--path", "../base"]);
        let config = sb.read("repo/.devset/config.toml");
        sb.write("repo/.devset/config.toml", &format!("# layers, in order\n{config}"));
        log += &sb.devset("repo", &["init", "--path", "../extra"]);
        for file in ["config.toml", "state.toml", ".gitignore", ".gitattributes"] {
            write!(log, "--- .devset/{file}\n{}", sb.read(&format!("repo/.devset/{file}")))
                .unwrap();
        }
        sb.assert(&log, snapbox::file!["snapshots/state_lives_in_dot_devset.txt"]);
    }

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
        sb.write(
            "missing/profile.toml",
            "[profile]\nname = \"missing\"\n\n[files.\"gone.toml\"]\n",
        );
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
    fn status_json() {
        let sb = Sandbox::new();
        sb.profile(
            "profile",
            "demo",
            &[("a.toml", "owned", "a = 1\n"), ("b.toml", "merge", "b = 1\n")],
        );
        sb.devset("repo", &["init", "--path", "../profile"]);
        sb.write("repo/a.toml", "a = 2\n");
        sb.assert(
            &sb.devset("repo", &["status", "--json"]),
            snapbox::file!["snapshots/status_json.txt"],
        );
    }

    #[test]
    fn schemas() {
        let sb = Sandbox::new();
        sb.assert(
            &sb.devset(".", &["schema", "profile"]),
            snapbox::file!["snapshots/schema_profile.txt"],
        );
        sb.assert(
            &sb.devset(".", &["schema", "config"]),
            snapbox::file!["snapshots/schema_config.txt"],
        );
    }

    #[test]
    fn update_merges_local_edits() {
        let sb = Sandbox::new();
        sb.publish(
            "p",
            "p",
            &[("deny.toml", "merge", "[licenses]\nallow = [\"MIT\"]\n\n[bans]\ndeny = []\n")],
            "v1",
        );
        let mut log = sb.devset(
            "repo",
            &["init", "--git", "../profiles.git", "--branch", "main", "--path", "p"],
        );
        sb.write(
            "repo/deny.toml",
            "[licenses]\nallow = [\"MIT\"]\n\n[bans]\ndeny = [\"openssl\"]\n",
        );
        sb.publish(
            "p",
            "p",
            &[(
                "deny.toml",
                "merge",
                "[licenses]\nallow = [\"MIT\", \"Zlib\"]\n\n[bans]\ndeny = []\n",
            )],
            "v2",
        );
        log += &sb.devset("repo", &["update", "--dry-run"]);
        log += &sb.devset("repo", &["update"]);
        assert_eq!(
            sb.read("repo/deny.toml"),
            "[licenses]\nallow = [\"MIT\", \"Zlib\"]\n\n[bans]\ndeny = [\"openssl\"]\n",
            "both edits survive"
        );
        log += &sb.devset("repo", &["status"]);
        log += &sb.devset("repo", &["apply"]);
        sb.assert(&log, snapbox::file!["snapshots/update_merges_local_edits.txt"]);
    }

    #[test]
    fn conflicts_wait_in_sidecars() {
        let sb = Sandbox::new();
        sb.publish(
            "p",
            "p",
            &[("a.toml", "merge", "x = 1\n"), ("b.toml", "owned", "b = 1\n")],
            "v1",
        );
        let mut log = sb.devset(
            "repo",
            &["init", "--git", "../profiles.git", "--branch", "main", "--path", "p"],
        );
        sb.write("repo/a.toml", "x = 2\n");
        sb.publish(
            "p",
            "p",
            &[("a.toml", "merge", "x = 3\n"), ("b.toml", "owned", "b = 2\n")],
            "v2",
        );
        let github = [("GITHUB_ACTIONS", "true")];
        log += &sb.devset_with(&github, "repo", &["update", "--dry-run"]);
        assert!(!sb.path("repo/.devset/conflicts").exists(), "a dry run writes no sidecar");
        log += &sb.devset("repo", &["update"]);
        assert_eq!(sb.read("repo/a.toml"), "x = 2\n", "the working file is untouched");
        assert_eq!(sb.read("repo/b.toml"), "b = 2\n", "clean files are applied");
        log += &sb.devset("repo", &["status", "--exit-code"]);
        log += &sb.devset("repo", &["apply"]);
        log += &sb.devset("repo", &["update", "--continue"]);
        sb.write("repo/.devset/conflicts/a.toml", "x = 23\n");
        log += &sb.devset("repo", &["update", "--continue"]);
        assert_eq!(sb.read("repo/a.toml"), "x = 23\n", "the resolution is installed");
        assert!(!sb.path("repo/.devset/conflicts").exists(), "sidecars are cleared");
        log += &sb.devset("repo", &["update", "--continue"]);
        sb.assert(&log, snapbox::file!["snapshots/conflicts_wait_in_sidecars.txt"]);
    }

    #[test]
    fn apply_none_withholds_everything() {
        let sb = Sandbox::new();
        sb.publish(
            "p",
            "p",
            &[("a.toml", "merge", "x = 1\n"), ("b.toml", "owned", "b = 1\n")],
            "v1",
        );
        let mut log = sb.devset(
            "repo",
            &["init", "--git", "../profiles.git", "--branch", "main", "--path", "p"],
        );
        sb.write(
            "repo/.devset/config.toml",
            &format!(
                "{}\n[merge]\non-conflict = \"apply-none\"\n",
                sb.read("repo/.devset/config.toml")
            ),
        );
        sb.write("repo/a.toml", "x = 2\n");
        sb.publish(
            "p",
            "p",
            &[("a.toml", "merge", "x = 3\n"), ("b.toml", "owned", "b = 2\n")],
            "v2",
        );
        log += &sb.devset("repo", &["update", "--dry-run"]);
        assert!(!sb.path("repo/.devset/pending.toml").exists(), "a dry run withholds nothing");
        log += &sb.devset("repo", &["update"]);
        assert_eq!(sb.read("repo/b.toml"), "b = 1\n", "nothing else is written");
        assert!(sb.path("repo/.devset/pending.toml").exists(), "the update waits in pending.toml");
        sb.write("repo/.devset/conflicts/a.toml", "x = 23\n");
        log += &sb.devset("repo", &["update", "--continue"]);
        assert_eq!(
            (sb.read("repo/a.toml"), sb.read("repo/b.toml")),
            ("x = 23\n".into(), "b = 2\n".into()),
            "all applied"
        );
        assert!(!sb.path("repo/.devset/pending.toml").exists(), "pending lock removed");
        sb.assert(&log, snapbox::file!["snapshots/apply_none_withholds_everything.txt"]);
    }

    #[test]
    fn invalid_merges_conflict() {
        let sb = Sandbox::new();
        sb.publish(
            "p",
            "p",
            &[("c.toml", "merge", "name = \"app\"\nreplicas = 1\nport = 80\n")],
            "v1",
        );
        let mut log = sb.devset(
            "repo",
            &["init", "--git", "../profiles.git", "--branch", "main", "--path", "p"],
        );
        sb.write("repo/c.toml", "name = \"app\"\ntimeout = 30\nreplicas = 1\nport = 80\n");
        sb.publish(
            "p",
            "p",
            &[("c.toml", "merge", "name = \"app\"\nreplicas = 1\nport = 80\ntimeout = 60\n")],
            "v2",
        );
        log += &sb.devset("repo", &["update"]);
        assert_eq!(
            sb.read("repo/c.toml"),
            "name = \"app\"\ntimeout = 30\nreplicas = 1\nport = 80\n",
            "never installed"
        );
        log += &sb.devset("repo", &["update", "--continue"]);
        sb.assert(&log, snapbox::file!["snapshots/invalid_merges_conflict.txt"]);
    }

    #[test]
    fn settings_precedence() {
        let sb = Sandbox::new();
        let layers = "[[layers]]\npath = \"../p\"\n\n[[layers]]\npath = \"../q\"\n";
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
        log += &sb.devset("repo", &["init", "--path", "../q"]);
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

    #[test]
    fn update_one_layer() {
        let sb = Sandbox::new();
        sb.publish("a", "a", &[("a.txt", "owned", "a1\n")], "v1");
        sb.profile("work/b", "b", &[("b.txt", "owned", "b1\n")]);
        sb.git("work", &["add", "-A"]);
        sb.git("work", &["commit", "-qm", "b"]);
        sb.git("work", &["push", "-q", "../profiles.git", "main"]);
        let mut log = sb.devset("repo", &["init", "--git", "../profiles.git", "--path", "a"]);
        log += &sb.devset("repo", &["init", "--git", "../profiles.git", "--path", "b"]);
        sb.publish("a", "a", &[("a.txt", "owned", "a2\n")], "v2");
        sb.profile("work/b", "b", &[("b.txt", "owned", "b2\n")]);
        sb.git("work", &["commit", "-qam", "b2"]);
        sb.git("work", &["push", "-q", "../profiles.git", "main"]);
        log += &sb.devset("repo", &["update", "nope"]);
        log += &sb.devset("repo", &["update", "b"]);
        assert_eq!(
            (sb.read("repo/a.txt"), sb.read("repo/b.txt")),
            ("a1\n".into(), "b2\n".into()),
            "only b moved"
        );
        sb.assert(&log, snapbox::file!["snapshots/update_one_layer.txt"]);
    }

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
    fn layers_collide_until_the_target_chooses() {
        let sb = Sandbox::new();
        sb.profile("base", "base", &[("fmt.toml", "owned", "base\n")]);
        sb.profile(
            "rust",
            "rust",
            &[("fmt.toml", "owned", "rust\n"), ("rust.toml", "owned", "r\n")],
        );
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
    fn update_names_are_suggested() {
        let sb = Sandbox::new();
        sb.profile("p", "rust", &[("a.toml", "owned", "a\n")]);
        let mut log = sb.devset("repo", &["init", "--path", "../p"]);
        log += &sb.devset("repo", &["update", "rsut"]);
        sb.assert(&log, snapbox::file!["snapshots/update_names_are_suggested.txt"]);
    }

    #[test]
    fn quiet_and_github() {
        let sb = Sandbox::new();
        sb.profile("p", "p", &[("a.toml", "owned", "a\n"), ("b.toml", "owned", "b\n")]);
        let mut log = sb.devset("repo", &["-q", "init", "--path", "../p"]);
        sb.write("repo/a.toml", "edited\n");
        sb.profile(
            "p",
            "p",
            &[("a.toml", "owned", "a\n"), ("b.toml", "owned", "b\n"), ("c.toml", "owned", "c\n")],
        );
        sb.write("summary.md", "");
        let root = sb.root.to_str().unwrap().to_owned();
        let summary = sb.path("summary.md");
        let github = [
            ("GITHUB_ACTIONS", "true"),
            ("GITHUB_WORKSPACE", root.as_str()),
            ("GITHUB_STEP_SUMMARY", summary.to_str().unwrap()),
        ];
        log += &sb.devset_with(&github, "repo", &["status", "--exit-code"]);
        write!(log, "--- $GITHUB_STEP_SUMMARY\n{}", sb.read("summary.md")).unwrap();
        sb.assert(&log, snapbox::file!["snapshots/quiet_and_github.txt"]);
    }
}
