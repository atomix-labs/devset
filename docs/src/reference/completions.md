# `devset completions`

<!-- reference: written by `just fix-docs` from `devset completions --help` -->

```text
Print a shell completion script

Usage: devset completions [OPTIONS] <SHELL>

Arguments:
  <SHELL>  The shell [possible values: bash, elvish, fish, powershell, zsh]

Options:
  -h, --help  Print help

Global Options:
  -q, --quiet     Print only results and errors
      --no-input  Never prompt; fail with the flags to pass instead
      --no-color  Never colour output

Examples:
  devset completions bash > ~/.local/share/bash-completion/completions/devset
  devset completions zsh > ~/.zfunc/_devset
  devset completions fish > ~/.config/fish/completions/devset.fish
```
