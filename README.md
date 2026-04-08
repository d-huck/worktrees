# work

A git worktree manager that keeps worktrees organized under `~/.worktrees/<repo>/<name>`.

## Installation

```sh
cargo install --path .
```

Then add shell integration to your profile so `work cd` can change your shell's directory:

```zsh
# ~/.zshrc or ~/.bashrc
eval "$(work init zsh)"   # or bash

# ~/.config/fish/config.fish
work init fish | source
```

## Usage

### Add a worktree

```sh
# New branch named after the worktree
work add feature-x

# Check out an existing branch
work add feature-x main

# New branch with a different name
work add feature-x -b my-branch

# Detached HEAD
work add feature-x --detach
```

Worktrees are stored at `~/.worktrees/<repo>/<name>`.

### Navigate

```sh
work cd feature-x
```

### List

```sh
work list
work list --verbose
work list --porcelain
```

### Remove

```sh
work remove feature-x
work remove feature-x --force
```

### Lock / Unlock

```sh
work lock feature-x --reason "don't prune this"
work unlock feature-x
```

### Move

```sh
work move feature-x /some/other/path
```

### Prune stale entries

```sh
work prune
work prune --dry-run
work prune --expire 2.weeks.ago
```

### Repair

```sh
work repair
```

## Tab completion

Completion scripts for zsh, fish, and bash are built into the binary. Worktree names are completed dynamically from `~/.worktrees/<repo>/` for the current repository.

```sh
work completions zsh   # print zsh completion script
work completions fish  # print fish completion script
work completions bash  # print bash completion script
```

The `init` command already sources completions automatically, so no extra setup is needed if you're using `eval "$(work init zsh)"`.
