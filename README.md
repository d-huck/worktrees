# work

A git worktree manager that keeps worktrees organized under `~/.worktrees/<repo>/<name>`.

## Installation

```sh
make install
```

This builds the binary, installs it via `cargo install`, and runs `work setup` to install shell completions and the directory-change wrapper automatically — no manual shell config needed.

Requires Homebrew or oh-my-zsh for zero-config setup on macOS. For other setups, run `work setup zsh|fish|bash` manually and follow any printed instructions.

### fzf (optional)

Install [fzf](https://github.com/junegunn/fzf) to enable the interactive worktree picker (`work on` with no arguments).

```sh
brew install fzf
```

## Usage

### Switch to a worktree (primary command)

```sh
work on feature-x        # switch to worktree, creating it first if it doesn't exist
work on                  # open fzf picker — shows branch, dirty status, last commit
```

`work on` is the main entry point. It creates the worktree if it doesn't exist, then changes your shell's directory to it.

The fzf picker shows rich info for each worktree and works from anywhere — inside a repo (shows that repo's worktrees) or outside (shows all worktrees across all repos).

### Add a worktree without switching

```sh
work add feature-x                # new branch named after the worktree
work add feature-x main           # check out an existing branch
work add feature-x -b my-branch   # new branch with a different name
work add feature-x --detach       # detached HEAD
```

Worktrees are stored at `~/.worktrees/<repo>/<name>`.

### Change directory into an existing worktree

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

Completions for zsh, fish, and bash are installed automatically by `make install`. Worktree names are completed dynamically and include branch name, dirty status, and last commit message.

```sh
work on <TAB>   # shows: name  branch ! · last commit (time ago) [repo]
```

When inside a repo, only that repo's worktrees are shown. Outside a repo, all worktrees across all repos are shown with a `[repo]` label.
