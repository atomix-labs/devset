# Templates

A file is copied as it is unless its entry marks it a template. A template is
rendered with the target's answers to the profile's variables, so a profile need
not hard-code what differs between targets: an author, a repository's name, a
machine's build flags.

```toml
# profile.toml
[vars.author]
prompt = "Author name"

[vars.target_cpu]
prompt  = "ISA floor for x86-64 builds"
default = "x86-64-v2"

[files."Cargo.toml"]
policy   = "once"
template = true
```

```toml
# files/Cargo.toml
[package]
authors = ["{{ author }}"]
```

Templates are [MiniJinja](https://docs.rs/minijinja): `{{ name }}` is a
variable, and the rest of Jinja's syntax works too. A variable a template uses
that no layer declares is an error, never an empty string. A variable's name is
an identifier: letters, digits and `_`, not starting with a digit.

## Answers

devset asks for any variable not yet answered when it runs in a terminal,
offering the default. Elsewhere, or with `--no-input`, it takes each default and
says so, and a variable with no default is an error that names the `--var` flag
to pass:

```sh
devset init --path ../profiles/rust --var author=Ada
```

Every answer, defaults included, is kept in `.devset/answers.toml`, which you
commit, so a profile that later changes a default never changes a target's file
without asking. A variable a profile newly declares is asked on the next update.

Variables describe the target, so they share one namespace: `author` is one
question, answered once, and used by every layer that declares it. Layers that
give it different defaults are settled by the target's answer.

## Changing an Answer

`init`, `apply` and `update` take `--var name=value`, which replaces the answer;
a name no layer declares is an error that lists the ones they do. To devset, a
changed answer is a change to the profile's version of every file that uses it:
an untouched file is written again, and an edited `merge` file is merged.

An answer to a variable no layer declares any more, after `devset remove`, say,
leaves `answers.toml`, and devset notes which.
