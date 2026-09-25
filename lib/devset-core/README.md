# `devset-core`

The library behind [devset]: versioned file bundles, applied to a directory and
updated without losing local edits. The `devset` command line, in
[`bin/devset-cli`](../../bin/devset-cli), reads arguments and prints reports;
everything else is here.

## The Chain

Every operation is a prefix of one chain, and only its last step writes:

| Step      | Gives      | What it does                                         |
| --------- | ---------- | ---------------------------------------------------- |
| `resolve` | `Resolved` | reads the target's layers and composes them          |
| `survey`  | `Survey`   | compares every managed path with its record and disk |
| `plan`    | `Plan`     | decides what happens to each path, merging edits     |
| `commit`  | the steps  | writes the files, and the state last                 |

`devset status` stops after `survey`, and `devset apply --dry-run` after `plan`.

## Use

```rust
use devset_core::source::Source;
use devset_core::{Cache, Mode, Refresh, Target, commit, plan, resolve, survey};

let mut target = Target::open_or_new(&root.join("repo"))?;
target.add_layer(Source::Dir("../base".into()))?;

let resolved = resolve(&target, &Cache::user()?, Refresh::None)?;
let survey = survey(resolved, &target)?;
let plan = plan(survey, Mode::Apply, &target)?;
commit(plan, &target)?;
```

The crate's documentation runs this example as a test, and covers updating,
conflicts and taking an update back:

```sh
cargo doc -p devset-core --open
```

## Stability

devset-core is versioned with devset, and its public API is part of what devset
promises: a change that breaks it is a breaking change to devset. It is not on
crates.io yet.

## License

MIT, as the rest of devset: see [LICENSE](../../LICENSE).

[devset]: https://github.com/atomix-labs/devset
