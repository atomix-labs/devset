//! The sandbox a test runs in: its own directories, `HOME`, cache and git configuration.

use core::fmt::Write as _;
use std::ffi::OsStr;
use std::path::PathBuf;
use std::process::Command;
use std::{env, fs};

use snapbox::cmd::cargo_bin;
use snapbox::{Assert, Data, Redactions};
use tempfile::TempDir;

/// A hermetic sandbox with its own `HOME`, cache and git configuration.
pub(crate) struct Sandbox {
    /// Removed on drop.
    _dir: TempDir,
    /// `dir`, canonical, as devset sees it: under `/private/var` on macOS.
    pub(crate) root: PathBuf,
}

impl Sandbox {
    /// An empty sandbox.
    pub(crate) fn new() -> Self {
        let dir = TempDir::new().unwrap();
        let root = dir.path().canonicalize().unwrap();
        Self { _dir: dir, root }
    }

    /// Absolute path of `rel`.
    pub(crate) fn path(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    /// Writes `text` to `rel`, creating parent directories.
    pub(crate) fn write(&self, rel: &str, text: &str) {
        let path = self.path(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    /// Contents of `rel`.
    pub(crate) fn read(&self, rel: &str) -> String {
        fs::read_to_string(self.path(rel)).unwrap()
    }

    /// Writes a profile at `dir`: `(path, policy, content)` per file.
    pub(crate) fn profile(&self, dir: &str, name: &str, files: &[(&str, &str, &str)]) {
        let files: Vec<(&str, String, &str)> = files
            .iter()
            .map(|&(path, policy, content)| (path, format!("policy = \"{policy}\""), content))
            .collect();
        let files: Vec<(&str, &str, &str)> =
            files.iter().map(|(path, entry, content)| (*path, entry.as_str(), *content)).collect();
        self.entries(dir, name, &files);
    }

    /// Writes a profile at `dir`: `(path, entry, content)` per file, `entry` the lines of its
    /// `[files."path"]` table.
    pub(crate) fn entries(&self, dir: &str, name: &str, files: &[(&str, &str, &str)]) {
        let mut manifest = format!("[profile]\nname = \"{name}\"\nversion = \"1.0.0\"\n");
        for (path, entry, content) in files {
            write!(manifest, "\n[files.\"{path}\"]\n{entry}\n").unwrap();
            self.write(&format!("{dir}/files/{path}"), content);
        }
        self.write(&format!("{dir}/profile.toml"), &manifest);
    }

    /// Appends `toml` to the manifest of the profile at `dir`: its `[requires]`, `[features]`,
    /// `[scaffolds]` or more `[files]`.
    pub(crate) fn extend(&self, dir: &str, toml: &str) {
        let manifest = self.read(&format!("{dir}/profile.toml"));
        self.write(&format!("{dir}/profile.toml"), &format!("{manifest}\n{toml}"));
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
    pub(crate) fn devset(&self, cwd: &str, args: &[&str]) -> String {
        self.devset_with(&[], cwd, args)
    }

    /// Checks `log` against the snapshot `expected`, the sandbox's path as `[ROOT]`.
    ///
    /// `SNAPSHOTS=overwrite` updates the snapshot instead.
    pub(crate) fn assert(&self, log: &str, expected: Data) {
        let mut redactions = Redactions::new();
        redactions.insert("[ROOT]", self.root.clone()).unwrap();
        Assert::new().action_env("SNAPSHOTS").redact_with(redactions).eq(log.to_owned(), expected);
    }

    /// [`Sandbox::devset`] with `vars` added to the environment.
    pub(crate) fn devset_with(&self, vars: &[(&str, &str)], cwd: &str, args: &[&str]) -> String {
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
    pub(crate) fn git(&self, cwd: &str, args: &[&str]) -> String {
        let out = self.command("git", cwd, args).output().unwrap();
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).trim().to_owned()
    }

    /// Publishes `work/<dir>` as a tagged commit of the bare repository `profiles.git`.
    pub(crate) fn publish(&self, dir: &str, name: &str, files: &[(&str, &str, &str)], tag: &str) {
        self.profile(&format!("work/{dir}"), name, files);
        self.release(tag);
    }

    /// Publishes `work/` as it is, tagged `tag`, to the bare repository `profiles.git`.
    pub(crate) fn release(&self, tag: &str) {
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
    pub(crate) fn blobs(&self, target: &str) -> usize {
        fs::read_dir(self.path(&format!("{target}/.devset/base"))).unwrap().count()
    }
}
