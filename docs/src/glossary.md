# Glossary

The words the manual uses, each with the page that says more.

| Word        | Meaning                                                                                                                                                     |
| ----------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| answer      | A target's value for a variable, kept in `.devset/answers.toml`. [Templates](templates.md#answers)                                                          |
| base        | The bytes devset last wrote for a file: what an update merges against. [State](state.md#the-base)                                                           |
| block       | A marked stretch of a text file that a profile owns. [Parts of a File](parts.md#blocks)                                                                     |
| bundle      | A profile with nothing but requirements. [Composing Profiles](composing.md#requirements)                                                                    |
| collection  | A source published for others, which may describe itself in `collection.toml`. [Publish a Collection](publish-a-collection.md)                              |
| conflict    | Where a profile's change and a local edit touch the same lines; left beside the file, in `.devset/conflicts/`. [Update and Resolve Conflicts](resolving.md) |
| drift       | A file devset manages that no longer matches what the profile says. [In CI](ci.md)                                                                          |
| feature     | An optional, additive capability of a profile. [Composing Profiles](composing.md#features)                                                                  |
| gate        | An entry's `when`: the conditions under which it applies. [Gates and Scaffolds](gates.md)                                                                   |
| keys        | The keys of a TOML, JSON or YAML file that a profile owns. [Parts of a File](parts.md#keys)                                                                 |
| layer       | One active profile of a target: configured by it, or required by a profile. [Composing Profiles](composing.md#layers)                                       |
| lock        | `.devset/lock.toml`: the commit each source resolved to, and each layer's features. [State](state.md)                                                       |
| part        | The keys, or the block, a profile owns in a file the target otherwise owns. [Parts of a File](parts.md)                                                     |
| policy      | How devset treats a file's local edits: `owned`, `merge` or `once`. [Profiles](profiles.md#policies)                                                        |
| profile     | A versioned bundle of files: `profile.toml`, and `files/` beside it. [Profiles](profiles.md)                                                                |
| requirement | A profile another builds on, by name. [Composing Profiles](composing.md#requirements)                                                                       |
| scaffold    | A group of starter files, written once when the target has none of its own. [Gates and Scaffolds](gates.md#scaffolds)                                       |
| source      | Where profiles come from: a git repository at a ref, or a directory. [Composing Profiles](composing.md#sources)                                             |
| starter     | A file's first content, written where the target has none, before a profile's part joins it. [Parts of a File](parts.md#starters)                           |
| target      | The directory devset applies profiles to, a repository or not. [State](state.md#finding-the-target)                                                         |
| template    | A file devset renders with the target's answers and its graph before writing it. [Templates](templates.md)                                                  |
| variable    | A value a profile asks the target for. [Templates](templates.md)                                                                                            |
