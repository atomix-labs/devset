# Templates: Library, Command Line, Workspace, Crate

Read this before writing a README or any section of one. Each template below is
a README whole, in the order the body's rules give: take the one for what the
repository builds, keep its order, fill each section from the code and a run,
and drop a section with nothing to say. `<owner>` is the repository's owner on
GitHub.

Every link to a file of the repository is absolute, since crates.io shows the
README too.

Contents: 1 Library · 2 Command Line · 3 Workspace · 4 Each Crate's Own README ·
5 How Crates.io Shows a README

## 1 Library

The header comes first, as the body's rules keep it; the pitch picks up where
the tagline, "Boards of tiles, and the moves across them.", stops; the example
is the crate page's first doctest, written out as a program.

````md
<!-- >>> devset: project >>> -->

(the header block, as the project profile renders it)

<!-- <<< devset: project <<< -->

A board of any size, each square named by its column and row, and a move off the
edge refused rather than wrapped. It draws nothing: a board is data, for a game
or a terminal to show.

## Install

```sh
cargo add tiles
```

`tiles` builds with Rust 1.98 or later.

## Quick Start

```rust
use tiles::{Board, Pos, PosError};

fn main() -> Result<(), PosError> {
    let board = Board { cols: 8, rows: 8 };
    assert_eq!(board.pos(3, 1)?, Pos { col: 3, row: 1 }, "a square of the board");
    assert_eq!(board.pos(8, 1), Err(PosError::Col { col: 8, cols: 8 }), "and not past it");
    Ok(())
}
```

## Features

| Feature | Adds                                                |
| ------- | --------------------------------------------------- |
| `serde` | `Serialize` and `Deserialize` for `Board` and `Pos` |

None is on by default: `cargo add tiles --features serde`.

## Documentation

- [The API on docs.rs][docs.rs], with an example for each type.
- [CHANGELOG.md][changelog]: what changed in each release.

## Contributing

Issues and pull requests are welcome: read [CONTRIBUTING.md][contributing]
first.

## License

Either [the MIT License][mit] or [the Apache License, Version 2.0][apache], at
your option.

[docs.rs]: https://docs.rs/tiles
[changelog]: https://github.com/<owner>/tiles/blob/main/CHANGELOG.md
[contributing]: https://github.com/<owner>/tiles/blob/main/CONTRIBUTING.md
[mit]: https://github.com/<owner>/tiles/blob/main/LICENSE-MIT
[apache]: https://github.com/<owner>/tiles/blob/main/LICENSE-APACHE
````

The minimum Rust is the workspace's `rust-version`, 1.98 where it is the
default; the feature table has the rows of the crate docs' `# Crate features`,
and a crate with no features has no such section.

Where the repository has a book, Documentation links it first, "The book: what
it teaches", to `https://<owner>.github.io/tiles/`, the site the header's book
badge links.

## 2 Command Line

The pitch says what the command does to what, and what it never does; Install
names the package, which differs from the binary; Quick Start is one command and
the output a run printed.

````md
<!-- >>> devset: project >>> -->

(the header block, as the project profile renders it)

<!-- <<< devset: project <<< -->

<!-- dprint-ignore-start -->

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/<owner>/tiles/main/docs/src/media/demo-dark.gif">
    <img alt="tiles shows a board, moves a tile two squares right, and refuses a move off the edge" src="https://raw.githubusercontent.com/<owner>/tiles/main/docs/src/media/demo-light.gif" width="720">
  </picture>
</p>

<!-- dprint-ignore-end -->

`tiles` shows a board of tiles in the terminal, and moves a tile across it. It
reads the board from a text file, one line a row, and writes the moved board to
standard output: it never changes the file.

## Install

```sh
cargo install --locked tiles-cli
```

The package is `tiles-cli`, and the binary `tiles`; `cargo install` builds it
with Rust 1.98 or later.

Each release also carries it prebuilt, for Linux and macOS, on
[the GitHub Release][release].

## Quick Start

```sh
tiles show board.txt
```

```text
  a b c d
1 . # . .
2 . . . #
```

## Usage

| Command                         | Does                                          |
| ------------------------------- | --------------------------------------------- |
| `tiles show <file>`             | draws the board                               |
| `tiles move <file> <from> <to>` | moves a tile, refusing a square off the board |

`tiles --help` lists every command and flag.

## Documentation

- [CHANGELOG.md][changelog]: what changed in each release.

## Contributing

Issues and pull requests are welcome: read [CONTRIBUTING.md][contributing]
first.

## License

Either [the MIT License][mit] or [the Apache License, Version 2.0][apache], at
your option.

[changelog]: https://github.com/<owner>/tiles/blob/main/CHANGELOG.md
[contributing]: https://github.com/<owner>/tiles/blob/main/CONTRIBUTING.md
[mit]: https://github.com/<owner>/tiles/blob/main/LICENSE-MIT
[apache]: https://github.com/<owner>/tiles/blob/main/LICENSE-APACHE
[release]: https://github.com/<owner>/tiles/releases/latest
````

The output is pasted from a run of the build the README describes, never typed;
the table names the commands a reader reaches for first, and the help has the
rest.

The demo stands under the header, in the `<picture>` vhs gives; the `demo`
workflow records the GIFs again into the same files at each release, and the alt
text says what `docs/demo/parts/demo.tape` shows.

## 3 Workspace

A library and the command line built on it: the root README is the project's
page, and names each crate; each crate's install line is its own.

````md
<!-- >>> devset: project >>> -->

(the header block, as the project profile renders it)

<!-- <<< devset: project <<< -->

A library and a command line: the library holds the board and refuses a move off
its edge; the command line reads a board from a file and shows it.

| Crate       | What it is                                                    |
| ----------- | ------------------------------------------------------------- |
| `tiles`     | the library: boards, squares and moves, [on docs.rs][docs.rs] |
| `tiles-cli` | the `tiles` command, which shows a board and moves its tiles  |

## Install

The library:

```sh
cargo add tiles
```

The command line, whose package is `tiles-cli` and whose binary is `tiles`:

```sh
cargo install --locked tiles-cli
```

Both build with Rust 1.98 or later.

## Quick Start

(the library's example, then the command line's, as in §1 and §2)

## Usage

(the command line's table, as in §2)

## Documentation

- [The library's API on docs.rs][docs.rs], with an example for each type.
- [CHANGELOG.md][changelog]: what changed in each release.

## Contributing

(as in §1)

## License

(as in §1)

[docs.rs]: https://docs.rs/tiles
[changelog]: https://github.com/<owner>/tiles/blob/main/CHANGELOG.md
````

The pitch is the project's, and the table says which crate is which, since a
reader looking for the library would otherwise install the command line. Only
the root README has the header.

## 4 Each Crate's Own README

One crate shows the root README, through `readme = "../../README.md"`: the one
the root README's first install line is for, `tiles`, the library, by default.
Every other published crate has a `README.md` beside its `Cargo.toml`, which
Cargo finds with no `readme` key: its name as the title, the crate's place in
the project in one paragraph, its install line and first use, its docs, and the
licence.

That crate is the one the `crate` variable names, whose crates.io and docs.rs
badges the header carries.

````md
# `tiles-cli`

The command line of [tiles][project]: the `tiles` command shows a board of tiles
in the terminal, and moves a tile across it. It is built on the `tiles` library.

## Install

```sh
cargo install --locked tiles-cli
```

The binary is `tiles`, and it builds with Rust 1.98 or later.

(the Quick Start of §2)

`tiles --help` lists every command and flag.

## License

Either [the MIT License][mit] or [the Apache License, Version 2.0][apache], at
your option.

[project]: https://github.com/<owner>/tiles
[mit]: https://github.com/<owner>/tiles/blob/main/LICENSE-MIT
[apache]: https://github.com/<owner>/tiles/blob/main/LICENSE-APACHE
````

A crate's README has no header and no badges: the root README carries them.

## 5 How Crates.io Shows a README

### Every Image and Link Is Absolute Where Crates.io Shows the README

crates.io shows, on each version's page, the README cargo uploaded when that
version was published. It rewrites a relative `href` or `src` only where the
manifest's `repository` is on github.com, gitlab.com or bitbucket.org, to that
repository's default branch, `blob/HEAD/<path>` for a link and `raw/HEAD/<path>`
for an image, and drops it on any other host. It joins the path to the README's
directory within the repository as the package records it, and a README cargo
copied in from outside the crate, through `readme = "../../README.md"`, is
recorded in the crate's own directory: the root README's `docs/media/board.svg`
becomes `crates/tiles/docs/media/board.svg`, which is no file. And it never
rewrites a `<source srcset>`, so a relative dark image resolves against
crates.io's own page. An absolute URL holds in every case, and wherever else the
README is copied.

```md
<!-- Bad: on crates.io, the dark image and both links resolve to nothing. -->
<picture>
  <source
    media="(prefers-color-scheme: dark)"
    srcset="docs/media/board-dark.svg"
  >
  <img
    alt="A board of 8 by 8 squares, three of them tiled"
    src="docs/media/board-light.svg"
    width="480"
  >
</picture>

Read [CONTRIBUTING.md](CONTRIBUTING.md) first; the licence is
[MIT](LICENSE-MIT).
```

```md
<picture>
  <source
    media="(prefers-color-scheme: dark)"
    srcset="https://raw.githubusercontent.com/<owner>/tiles/main/docs/media/board-dark.svg"
  >
  <img
    alt="A board of 8 by 8 squares, three of them tiled"
    src="https://raw.githubusercontent.com/<owner>/tiles/main/docs/media/board-light.svg"
    width="480"
  >
</picture>

Read [CONTRIBUTING.md][contributing] first; the licence is [MIT][mit].

[contributing]: https://github.com/<owner>/tiles/blob/main/CONTRIBUTING.md
[mit]: https://github.com/<owner>/tiles/blob/main/LICENSE-MIT
```

Held by review: `just check-cargo-publish` packages each crate and builds it,
and reads no link in its README.

### Every Published Crate Has a README Crates.io Shows

Cargo packages the file `readme` names, or, with no `readme`, a `README.md`,
`README.txt` or `README` in the crate's own directory. A crate in
`crates/<name>/` with neither is published with no README, and its page on
crates.io shows none.

```toml
# Bad: no `readme`, and no README beside this file, so crates.io shows none.
[package]
name                   = "tiles"
description            = "boards of tiles, and the moves across them."
version.workspace      = true
edition.workspace      = true
rust-version.workspace = true
license.workspace      = true
authors.workspace      = true
repository.workspace   = true

[lints]
workspace = true
```

```toml
[package]
name                   = "tiles"
description            = "boards of tiles, and the moves across them."
version.workspace      = true
edition.workspace      = true
rust-version.workspace = true
license.workspace      = true
authors.workspace      = true
repository.workspace   = true
readme                 = "../../README.md"

[lints]
workspace = true
```

Held by review: `cargo package -p tiles --list` lists `README.md` where the
crate has one; `just check-cargo-publish` fails only a `readme` that names no
file.
