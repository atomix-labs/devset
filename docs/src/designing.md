# Designing Profiles

A profile is a crate for a repository, and the choices are a crate author's:
what is one crate and what two, what is a feature, what a configuration value.
This chapter says how to make them, so a collection stays small, clear, and easy
to take a piece of.

## The Units

| Unit                 | Crate analogy       | Use it for                                                       |
| -------------------- | ------------------- | ---------------------------------------------------------------- |
| Source               | workspace, registry | profiles released together under one ref                         |
| Profile              | crate               | a concern a repository adopts, updates or removes as a unit      |
| Feature              | Cargo feature       | an optional, additive capability of that concern                 |
| Variable             | configuration value | a value per repository, or a choice between alternatives         |
| Requirement          | dependency          | a concern this one cannot work without                           |
| Optional requirement | optional dependency | a capability that needs another concern: `["dep:lychee"]`        |
| Bundle               | facade crate        | only requirements, with features that describe a kind of project |
| Scaffold             | a template          | starter content that becomes the project's once written          |
| Managed entry        | a dependency's code | content that stays in sync with the source                       |

## The Tests

Ask them in order:

1. **Would a repository want one without the other?** Then two profiles. If they
   always go together and edit the same files, one.
2. **Would turning it on remove or replace something?** Then a variable, not a
   feature: features only add, and two can always be on at once.
3. **Does the description join unrelated things with "and"?** Then two profiles.
4. **Does it only make sense with its concern?** Then a feature of that concern,
   not a profile of its own.
5. **Is it content the project will own and change?** Then a scaffold. Content
   that must follow the source is managed: owned, merged, or a part.
6. **Names are API.** Removing a profile, a feature or a variable, or narrowing
   what a feature adds, breaks the targets that name it: release it as a
   breaking change.

## Working Examples

- **A book.** One `mdbook` profile: its scaffold writes the book when the
  project has none, its managed keys keep `book.toml` wired up either way, and
  KaTeX, Mermaid and an API reference are features, since each only makes sense
  with a book. Link checking is another concern, an optional requirement:
  `links = ["dep:lychee"]`.
- **A licence.** `MIT`, `Apache-2.0` and both are alternatives, so a variable,
  `license`, with each licence file gated on its answers; not three features,
  since turning one on would have to turn another off.
- **A Rust project.** A `rust` bundle requires the concerns every Rust
  repository has, and its features name kinds of project: `docs` turns on the
  book, `publish` the crates.io profile, `binaries` the release archives.
- **House rules.** Rules one team wants and others would not are a `strict`
  feature of each concern, not a fork of the collection.

## Adopting What Is There

A profile adopting an existing repository should keep what it finds, and wire it
up. The tools:

- A **part**, `scope = "keys"` or `"block"`, manages what the profile needs in a
  file the project owns.
- A **scaffold** writes starters only where the project has none, and remembers
  that it found some.
- A **gate** on `exists` attaches to what the project, or another profile, has:
  a block in `AGENTS.md` only where there is an `AGENTS.md`.
- A **starter** composes with parts, so the same profiles create a file from
  nothing and fill in an existing one.

## Profiles for Agents

A concern that teaches an agent how to work with what it sets up ships that
teaching with itself, behind an `agents` feature: a skill beside the tool it is
about, gated so a project that does not use agents never gets it. A template can
name only the recipes the graph has, through `devset.profiles`.
