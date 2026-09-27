# Examples

Each example is a directory to run in, and a README with the commands and what
they print. A test runs every README's commands and compares what they print, so
an example that stops working fails CI. None needs the network.

| Example                                                    | Shows                                                        |
| ---------------------------------------------------------- | ------------------------------------------------------------ |
| [`first-profile`](first-profile/README.md)                 | A profile applied to a repository; drift, and an edit kept   |
| [`local-edits-kept`](local-edits-kept/README.md)           | A profile's change merged with the repository's own edit     |
| [`part-of-a-file`](part-of-a-file/README.md)               | Owning some keys of `Cargo.toml` and a block of `.gitignore` |
| [`features-and-gates`](features-and-gates/README.md)       | Files that apply with a feature, or where a file exists      |
| [`build-on-a-collection`](build-on-a-collection/README.md) | A team's profile requiring another collection's, by tag      |

Each starts with `git init`, and writes into its directory: run it in a copy.

```sh
cp -r examples/first-profile /tmp/ && cd /tmp/first-profile
```
