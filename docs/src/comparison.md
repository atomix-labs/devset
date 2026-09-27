# Comparison

Many tools start a project from a template; fewer keep it in step with the
template afterwards. devset is built for the second part: a repository takes
several small profiles, each owning whole files or only parts of them, and stays
current with each release, keeping its own edits, with drift failing CI.

Each cell below links the page that shows it, in the tool's own documentation,
or, where the documentation says nothing, its source. They were read in
September 2026, against copier 9.18, cruft 2.16, projen 0.103, chezmoi 2.72 and
cargo-generate 0.25. A cell that is wrong, or out of date, is a bug: a pull
request fixing it, with the page that shows it, is welcome.

|                                             | devset     | copier       | cruft        | projen       | chezmoi      | cargo-generate |
| ------------------------------------------- | ---------- | ------------ | ------------ | ------------ | ------------ | -------------- |
| Starts a new project                        | [yes][d1]  | [yes][c1]    | [yes][r1]    | [yes][p1]    | [no][z1]     | [yes][g1]      |
| Takes the template's later changes          | [yes][d2]  | [yes][c2]    | [yes][r2]    | [yes][p2]    | [yes][z2]    | [no][g2]       |
| Keeps the project's edits through them      | [yes][d3]  | [yes][c3]    | [yes][r3]    | [no][p3]     | [partly][z3] | [no][g2]       |
| Owns part of a file, the rest the project's | [yes][d4]  | [no][c4]     | [no][r2]     | [no][p4]     | [yes][z4]    | [no][g1]       |
| Several templates, each kept current        | [yes][d5]  | [yes][c5]    | [no][r4]     | [partly][p5] | [partly][z5] | [no][g2]       |
| A template builds on another                | [yes][d6]  | [no][c6]     | [no][r2]     | [yes][p6]    | [no][z5]     | [no][g3]       |
| Optional parts, by feature or answer        | [yes][d7]  | [yes][c7]    | [partly][r5] | [yes][p7]    | [yes][z6]    | [yes][g4]      |
| A drift check for CI                        | [yes][d8]  | [no][c8]     | [yes][r6]    | [yes][p8]    | [yes][z7]    | [no][g2]       |
| Applies without running the template's code | [yes][d9]  | [partly][c9] | [no][r7]     | [no][p9]     | [partly][z8] | [partly][g5]   |
| Pins the template's version                 | [yes][d10] | [yes][c10]   | [yes][r8]    | [yes][p10]   | [partly][z9] | [yes][g6]      |

Where a change meets an edit, the tools differ most:

- **devset** writes the conflict beside the file, in `.devset/conflicts/`, and
  leaves the working file as it was, so a formatter or a build never meets
  conflict markers; `devset apply --continue` installs the resolution, and
  `--abort` takes the run back. [Updating and Merging](updating.md#conflicts)
- **copier** writes conflict markers into the file by default, or `.rej` files
  beside it with `--conflict rej`.
  [copier](https://copier.readthedocs.io/en/stable/configuring/#conflict)
- **cruft** applies the template's diff with `git apply -3`, which writes
  markers into the file; outside a git repository it falls back to `.rej` files.
  [cruft](https://github.com/cruft/cruft/blob/master/cruft/_commands/update.py#L194-L278)
- **projen** has nothing to merge: it writes its files again from code, and a
  hand edit fails its anti-tamper check.
  [projen](https://projen.io/docs/api/projen#projen.FileBaseOptions.property.committed)
- **chezmoi** asks before overwriting a file changed since it wrote it, and
  `chezmoi merge` opens a three-way merge tool.
  [chezmoi](https://www.chezmoi.io/reference/commands/merge/)

## What Each Is For

In their own words:

- copier: "a code lifecycle management tool."
  ([comparisons](https://copier.readthedocs.io/en/stable/comparisons/))
- cruft: "maintain all the necessary boilerplate for packaging and building
  projects separate from the code you intentionally write."
  ([key features](https://cruft.github.io/cruft/#key-features))
- projen: "Define and maintain complex project configuration through code."
  ([README](https://github.com/projen/projen#readme))
- chezmoi: "Manage your dotfiles across multiple diverse machines, securely."
  ([chezmoi.io](https://www.chezmoi.io/))
- cargo-generate: "get up and running quickly with a new Rust project by
  leveraging a pre-existing git repository as a template."
  ([book](https://cargo-generate.github.io/cargo-generate/))

## When You Do Not Need devset

One repository, sharing nothing with another, has nothing for devset to keep in
step: a template tool, or no tool, serves it. A project started once from a
template and never updated from it is cargo-generate's or copier's. A home
directory's dotfiles are chezmoi's. Configuration written as code, whole files
generated and never edited, is projen's. devset earns its place when several
repositories share configuration that each also edits.

[d1]: new-project.md
[d2]: resolving.md#taking-a-change
[d3]: updating.md#merging
[d4]: parts.md
[d5]: composing.md#layers
[d6]: composing.md#requirements
[d7]: gates.md
[d8]: ci.md
[d9]: introduction.md#how
[d10]: state.md
[c1]: https://copier.readthedocs.io/en/stable/comparisons/
[c2]: https://copier.readthedocs.io/en/stable/updating/#how-the-update-works
[c3]: https://copier.readthedocs.io/en/stable/updating/#how-the-update-works
[c4]: https://copier.readthedocs.io/en/stable/configuring/#skip_if_exists
[c5]: https://copier.readthedocs.io/en/stable/configuring/#applying-multiple-templates-to-the-same-subproject
[c6]: https://copier.readthedocs.io/en/stable/configuring/#include-other-yaml-files
[c7]: https://copier.readthedocs.io/en/stable/configuring/#conditional-files-and-directories
[c8]: https://copier.readthedocs.io/en/stable/updating/#checking-for-updates
[c9]: https://copier.readthedocs.io/en/stable/configuring/#unsafe
[c10]: https://copier.readthedocs.io/en/stable/configuring/#vcs_ref
[r1]: https://cruft.github.io/cruft/#creating-a-new-project
[r2]: https://cruft.github.io/cruft/#updating-a-project
[r3]: https://github.com/cruft/cruft/blob/master/cruft/_commands/update.py#L194-L278
[r4]: https://github.com/cruft/cruft/blob/master/cruft/_commands/utils/cruft.py#L18
[r5]: https://cookiecutter.readthedocs.io/en/stable/advanced/boolean_variables.html
[r6]: https://cruft.github.io/cruft/#checking-a-project
[r7]: https://cookiecutter.readthedocs.io/en/stable/advanced/hooks.html
[r8]: https://cruft.github.io/cruft/#creating-a-new-project
[p1]: https://projen.io/docs/introduction/the-projen-workflow#initializing-a-project
[p2]: https://projen.io/docs/introduction/the-projen-workflow#applying-changes
[p3]: https://projen.io/docs/concepts/escape-hatches#object-file-patches
[p4]: https://projen.io/docs/concepts/escape-hatches#object-file-patches
[p5]: https://projen.io/docs/concepts/projects/sub-projects
[p6]: https://projen.io/docs/concepts/projects/building-your-own#creating-the-project-type
[p7]: https://projen.io/docs/faq#how-do-i-specify-parameters-when-i-create-a-project
[p8]: https://projen.io/docs/api/projen#projen.FileBaseOptions.property.committed
[p9]: https://projen.io/docs/concepts/ejecting
[p10]: https://projen.io/docs/api/javascript#projen.javascript.NodeProjectOptions.property.projenVersion
[z1]: https://www.chezmoi.io/reference/commands/init/
[z2]: https://www.chezmoi.io/reference/commands/update/
[z3]: https://www.chezmoi.io/reference/commands/apply/
[z4]: https://www.chezmoi.io/user-guide/manage-different-types-of-file/#manage-part-but-not-all-of-a-file
[z5]: https://www.chezmoi.io/user-guide/frequently-asked-questions/design/#can-chezmoi-support-multiple-sources-or-multiple-source-states
[z6]: https://www.chezmoi.io/user-guide/manage-machine-to-machine-differences/#ignore-files-or-a-directory-on-different-machines
[z7]: https://www.chezmoi.io/reference/commands/verify/
[z8]: https://www.chezmoi.io/user-guide/use-scripts-to-perform-actions/#understand-how-scripts-work
[z9]: https://www.chezmoi.io/reference/commands/init/#-tag-tag
[g1]: https://cargo-generate.github.io/cargo-generate/
[g2]: https://github.com/cargo-generate/cargo-generate/blob/v0.25.0/src/args.rs#L168-L262
[g3]: https://cargo-generate.github.io/cargo-generate/usage/index.html#templates-in-subfolders
[g4]: https://cargo-generate.github.io/cargo-generate/templates/conditional.html
[g5]: https://cargo-generate.github.io/cargo-generate/templates/scripting.system-commands.html
[g6]: https://cargo-generate.github.io/cargo-generate/favorites.html
