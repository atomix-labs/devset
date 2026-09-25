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
            let files: Vec<(&str, String, &str)> = files
                .iter()
                .map(|&(path, policy, content)| (path, format!("policy = \"{policy}\""), content))
                .collect();
            let files: Vec<(&str, &str, &str)> = files
                .iter()
                .map(|(path, entry, content)| (*path, entry.as_str(), *content))
                .collect();
            self.entries(dir, name, &files);
        }

        /// Writes a profile at `dir`: `(path, entry, content)` per file, `entry` the lines of its
        /// `[files."path"]` table.
        fn entries(&self, dir: &str, name: &str, files: &[(&str, &str, &str)]) {
            let mut manifest = format!("[profile]\nname = \"{name}\"\nversion = \"1.0.0\"\n");
            for (path, entry, content) in files {
                write!(manifest, "\n[files.\"{path}\"]\n{entry}\n").unwrap();
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
            self.devset_with(&[], cwd, args)
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
            self.release(tag);
        }

        /// Publishes `work/` as it is, tagged `tag`, to the bare repository `profiles.git`.
        fn release(&self, tag: &str) {
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
        assert!(!sb.path("repo/b.toml").exists(), "a released file as devset wrote it goes");
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
        sb.release("v1");

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
        assert!(!sb.path("repo/.devset/conflicts").exists(), "a dry run withholds nothing");
        log += &sb.devset("repo", &["update"]);
        assert_eq!(sb.read("repo/b.toml"), "b = 1\n", "nothing else is written");
        assert!(
            sb.path("repo/.devset/conflicts/.devset/pending.toml").exists(),
            "the update waits in its pending lock"
        );
        sb.write("repo/.devset/conflicts/a.toml", "x = 23\n");
        log += &sb.devset("repo", &["update", "--continue"]);
        assert_eq!(
            (sb.read("repo/a.toml"), sb.read("repo/b.toml")),
            ("x = 23\n".into(), "b = 2\n".into()),
            "all applied"
        );
        assert!(!sb.path("repo/.devset/conflicts").exists(), "pending lock removed");
        sb.assert(&log, snapbox::file!["snapshots/apply_none_withholds_everything.txt"]);
    }

    #[test]
    fn abort_takes_the_update_back() {
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
        let records = || (sb.read("repo/.devset/lock.toml"), sb.read("repo/.devset/state.toml"));
        let (before, bases) = (records(), sb.blobs("repo"));
        sb.publish(
            "p",
            "p",
            &[
                ("a.toml", "merge", "x = 3\n"),
                ("b.toml", "owned", "b = 2\n"),
                ("c.toml", "owned", "c = 1\n"),
            ],
            "v2",
        );
        log += &sb.devset("repo", &["update"]);
        log += &sb.devset("repo", &["update", "--abort", "--dry-run"]);
        sb.write("repo/b.toml", "b = 9\n");
        log += &sb.devset("repo", &["update", "--abort"]);
        assert_eq!(sb.read("repo/b.toml"), "b = 9\n", "a refused abort writes nothing");
        log += &sb.devset("repo", &["update", "--abort", "--force"]);
        assert_eq!(
            (sb.read("repo/a.toml"), sb.read("repo/b.toml")),
            ("x = 2\n".into(), "b = 1\n".into()),
            "every file as before the update"
        );
        assert!(!sb.path("repo/c.toml").exists(), "a file the update created is removed");
        assert_eq!((records(), sb.blobs("repo")), (before, bases), "and the lock, state and bases");
        assert!(!sb.path("repo/.devset/conflicts").exists(), "the conflicts are discarded");
        log += &sb.devset("repo", &["update", "--abort"]);
        sb.assert(&log, snapbox::file!["snapshots/abort_takes_the_update_back.txt"]);
    }

    #[test]
    fn abort_discards_a_withheld_update() {
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
        let config = sb.read("repo/.devset/config.toml");
        sb.write(
            "repo/.devset/config.toml",
            &format!("{config}\n[merge]\non-conflict = \"apply-none\"\n"),
        );
        sb.write("repo/a.toml", "x = 2\n");
        let lock = sb.read("repo/.devset/lock.toml");
        sb.publish(
            "p",
            "p",
            &[("a.toml", "merge", "x = 3\n"), ("b.toml", "owned", "b = 2\n")],
            "v2",
        );
        log += &sb.devset("repo", &["update"]);
        log += &sb.devset("repo", &["update", "--abort"]);
        assert_eq!(sb.read("repo/.devset/lock.toml"), lock, "the lock never moved");
        assert!(!sb.path("repo/.devset/conflicts").exists(), "the withheld lock is discarded");
        log += &sb.devset("repo", &["status"]);
        sb.assert(&log, snapbox::file!["snapshots/abort_discards_a_withheld_update.txt"]);
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

    #[test]
    fn diff_shows_how_files_differ() {
        let sb = Sandbox::new();
        sb.profile(
            "p",
            "p",
            &[
                ("a.toml", "owned", "x = 1\n"),
                ("b.toml", "merge", "y = 1\n"),
                ("c.toml", "owned", "z = 1\n"),
                ("same.toml", "owned", "s = 1\n"),
            ],
        );
        sb.write("p/files/logo.png", "PNG\0one");
        let manifest = sb.read("p/profile.toml");
        sb.write("p/profile.toml", &format!("{manifest}\n[files.\"logo.png\"]\n"));
        let mut log = sb.devset("repo", &["init", "--path", "../p"]);
        log += &sb.devset("repo", &["diff"]);
        sb.write("repo/a.toml", "x = 2\n");
        sb.write("repo/b.toml", "y = 1\nw = 0\n");
        fs::remove_file(sb.path("repo/c.toml")).unwrap();
        sb.write("repo/same.toml", "s = 1   \r\n");
        sb.write("repo/logo.png", "PNG\0two");
        log += &sb.devset("repo", &["diff"]);
        log += &sb.devset("repo/sub", &["diff", "../b.toml"]);
        log += &sb.devset("repo", &["diff", "b.tml"]);
        sb.assert(&log, snapbox::file!["snapshots/diff_shows_how_files_differ.txt"]);
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

    #[test]
    fn unfinished_updates_fail_the_gate() {
        let sb = Sandbox::new();
        sb.publish(
            "p",
            "p",
            &[("a.toml", "merge", "x = 1\n"), ("b.toml", "owned", "b = 1\n")],
            "v1",
        );
        sb.devset("repo", &["init", "--git", "../profiles.git", "--branch", "main", "--path", "p"]);
        let config = sb.read("repo/.devset/config.toml");
        sb.write(
            "repo/.devset/config.toml",
            &format!("{config}\n[merge]\non-conflict = \"apply-none\"\n"),
        );
        sb.write("repo/a.toml", "x = 2\n");
        sb.publish(
            "p",
            "p",
            &[("a.toml", "merge", "x = 3\n"), ("b.toml", "owned", "b = 2\n")],
            "v2",
        );
        sb.devset("repo", &["update"]);
        let log = sb.devset("repo", &["status", "--exit-code"]);
        sb.assert(&log, snapbox::file!["snapshots/unfinished_updates_fail_the_gate.txt"]);
    }

    #[test]
    fn conflicts_without_a_merge() {
        let sb = Sandbox::new();
        sb.profile("p", "p", &[("m.toml", "merge", "a = 1\n"), ("o.toml", "owned", "o = 1\n")]);
        sb.write("p/files/b.dat", "B\0one");
        let manifest = sb.read("p/profile.toml");
        sb.write("p/profile.toml", &format!("{manifest}\n[files.\"b.dat\"]\npolicy = \"merge\"\n"));
        let mut log = sb.devset("repo", &["init", "--path", "../p"]);
        // A base store that was never committed, then cloned: the bases are gone.
        fs::remove_dir_all(sb.path("repo/.devset/base")).unwrap();
        sb.write("repo/m.toml", "a = 1\nb = 2\n");
        sb.write("p/files/m.toml", "z = 0\na = 1\n");
        sb.write("repo/b.dat", "B\0mine");
        sb.write("p/files/b.dat", "B\0theirs");
        sb.write("p/files/o.toml", "o = 2\n");
        log += &sb.devset("repo", &["apply"]);
        assert_eq!(sb.read("repo/o.toml"), "o = 2\n", "every other file proceeds");
        assert_eq!(sb.read("repo/.devset/conflicts/b.dat"), "B\0theirs", "the profile's version");
        write!(log, "--- .devset/conflicts/m.toml\n{}", sb.read("repo/.devset/conflicts/m.toml"))
            .unwrap();
        sb.assert(&log, snapbox::file!["snapshots/conflicts_without_a_merge.txt"]);
    }

    #[test]
    fn mistakes_are_named_with_their_fix() {
        let sb = Sandbox::new();
        sb.profile("profiles/rust", "rust", &[("a.toml", "owned", "a\n")]);
        sb.profile("profiles/base", "rust", &[("b.toml", "owned", "b\n")]);
        sb.publish("rust", "rust", &[("a.toml", "owned", "a\n")], "v1");
        sb.profile("t", "t", &[("t.txt", "owned", "hi {{ nmae }}\n")]);
        let manifest =
            sb.read("t/profile.toml").replace("policy = \"owned\"\n", "template = true\n");
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

    #[test]
    fn completions_are_printed() {
        let sb = Sandbox::new();
        let log = sb.devset(".", &["completions", "bash"]);
        assert!(log.contains("_devset()"), "a bash completion function");
        assert!(log.ends_with("[exit 0]\n"), "that succeeds");
    }

    #[test]
    fn dropped_answers_are_noted() {
        let sb = Sandbox::new();
        sb.profile("p", "p", &[("t.txt", "owned", "{{ cpu }}\n")]);
        let manifest =
            sb.read("p/profile.toml").replace("policy = \"owned\"\n", "template = true\n");
        sb.write("p/profile.toml", &format!("{manifest}\n[vars.cpu]\ndefault = \"x86-64-v2\"\n"));
        sb.profile("q", "q", &[("q.txt", "owned", "q\n")]);
        sb.devset("repo", &["init", "--path", "../p"]);
        let mut log = sb.devset("repo", &["init", "--path", "../q"]);
        log += &sb.devset("repo", &["remove", "p"]);
        assert!(!sb.path("repo/.devset/answers.toml").exists(), "no answers left");
        sb.assert(&log, snapbox::file!["snapshots/dropped_answers_are_noted.txt"]);
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
        sb.assert(
            &log,
            snapbox::file!["snapshots/requirements_are_refused_when_they_cannot_hold.txt"],
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
    fn executable_files_are_written_executable() {
        use std::os::unix::fs::PermissionsExt as _;
        let sb = Sandbox::new();
        let files = |script: &'static str| {
            [("setup.sh", "executable = true", script), ("notes.txt", "", "notes\n")]
        };
        sb.entries("tools", "tools", &files("#!/bin/sh\necho ready\n"));
        let mut log = sb.devset("repo", &["init", "--path", "../tools"]);
        let mode = |rel: &str| fs::metadata(sb.path(rel)).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode("repo/setup.sh"), 0o755, "an executable file is written 755");
        assert_eq!(mode("repo/notes.txt") & 0o111, 0, "and no other file is");
        sb.entries("tools", "tools", &files("#!/bin/sh\necho ready again\n"));
        log += &sb.devset("repo", &["apply"]);
        assert_eq!(mode("repo/setup.sh"), 0o755, "and it stays so when written again");
        fs::set_permissions(sb.path("repo/setup.sh"), fs::Permissions::from_mode(0o644)).unwrap();
        log += &sb.devset("repo", &["status", "--exit-code"]);
        sb.assert(&log, snapbox::file!["snapshots/executable_files_are_written_executable.txt"]);
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

    #[test]
    fn keys_merge_leaf_by_leaf() {
        let sb = Sandbox::new();
        let entry = "scope = \"keys\"\npolicy = \"merge\"";
        let v1 = "[bans]\nmultiple-versions = \"deny\"\nwildcards = \"deny\"\n\n[licenses]\nallow = [\"MIT\"]\nconfidence-threshold = 0.9\n";
        sb.entries("work/p", "p", &[("deny.toml", entry, v1)]);
        sb.release("v1");
        let mut log = sb.devset(
            "repo",
            &["init", "--git", "../profiles.git", "--branch", "main", "--path", "p"],
        );
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
        log += &sb.devset("repo", &["init", "--path", "../rust"]);
        write!(log, "--- Cargo.toml\n{}", sb.read("repo/Cargo.toml")).unwrap();
        log += &sb.devset("repo", &["status", "-v"]);
        log += &sb.devset("repo", &["init", "--path", "../strict"]);
        log += &sb.devset("repo", &["init", "--path", "../whole"]);
        let config = sb.read("repo/.devset/config.toml");
        let config = format!(
            "{config}\n[[layers]]\npath = \"../whole\"\n\n[files.\"Cargo.toml\"]\nfrom = \"lints\"\n"
        );
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
        sb.entries(
            "p",
            "p",
            &[("Cargo.toml", "scope = \"block\"", "[workspace]\nresolver = \"3\"\n")],
        );
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
}
