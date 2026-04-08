mod worktree;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Clone, ValueEnum)]
enum Shell {
    Zsh,
    Fish,
    Bash,
}

#[derive(Parser)]
#[command(name = "work", about = "Git worktree manager — stores worktrees in ~/.worktrees/<repo>/<name>")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Add a new worktree stored in ~/.worktrees/<repo>/<name>
    Add {
        /// Name of the worktree (becomes the directory name and default branch name)
        name: String,
        /// Commit, branch, or tag to check out
        commit_ish: Option<String>,
        /// Create and checkout a new branch with this name
        #[arg(short = 'b', long)]
        branch: Option<String>,
        /// Detach HEAD instead of checking out a branch
        #[arg(long)]
        detach: bool,
        /// Do not checkout files after creating the worktree
        #[arg(long)]
        no_checkout: bool,
    },
    /// List worktrees
    List {
        /// Use porcelain output format
        #[arg(long)]
        porcelain: bool,
        /// Verbose output
        #[arg(short, long)]
        verbose: bool,
    },
    /// Remove a worktree
    Remove {
        /// Name of the worktree to remove
        name: String,
        /// Force removal even with uncommitted changes or untracked files
        #[arg(short, long)]
        force: bool,
    },
    /// Print the path to a worktree (used by the shell function for `work cd`)
    Cd {
        /// Name of the worktree
        name: String,
    },
    /// Lock a worktree to prevent pruning
    Lock {
        /// Name of the worktree
        name: String,
        /// Reason for locking
        #[arg(long)]
        reason: Option<String>,
    },
    /// Unlock a worktree
    Unlock {
        /// Name of the worktree
        name: String,
    },
    /// Move a worktree to a new path
    Move {
        /// Name of the worktree
        name: String,
        /// Destination path
        new_path: String,
    },
    /// Repair worktree administrative files
    Repair {
        /// Path to repair (defaults to current worktree)
        path: Option<String>,
    },
    /// Prune worktree information in $GIT_DIR
    Prune {
        /// Do not remove, show only what would be pruned
        #[arg(short = 'n', long)]
        dry_run: bool,
        /// Report all removals
        #[arg(short, long)]
        verbose: bool,
        /// Only expire entries older than this (e.g. "now", "2.weeks.ago")
        #[arg(long)]
        expire: Option<String>,
    },
    /// Install shell completions and the cd wrapper to standard system locations
    ///
    /// For zsh, writes ~/.zfunc/_work and prints what to add to ~/.zshrc.
    /// For fish, writes to ~/.config/fish/{completions,functions}/ automatically.
    /// For bash, writes to ~/.local/share/bash-completion/completions/work.
    Setup {
        /// Shell to set up
        shell: Shell,
    },
    /// Print shell integration code (alternative to `setup` for manual config)
    ///
    /// Zsh:  eval "$(work init zsh)"
    /// Fish: work init fish | source
    /// Bash: eval "$(work init bash)"
    Init {
        /// Shell to generate integration for
        shell: Shell,
    },
    /// Generate shell tab-completion script
    Completions {
        /// Shell to generate completions for
        shell: Shell,
    },
    /// List worktree names for the current repo (used internally by completions)
    #[command(hide = true, name = "list-names")]
    ListNames,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Add {
            name,
            commit_ish,
            branch,
            detach,
            no_checkout,
        } => {
            worktree::add(&name, commit_ish.as_deref(), branch.as_deref(), detach, no_checkout)?;
        }
        Commands::List { porcelain, verbose } => {
            worktree::list(porcelain, verbose)?;
        }
        Commands::Remove { name, force } => {
            worktree::remove(&name, force)?;
        }
        Commands::Cd { name } => {
            // Print the path to stdout — the shell function captures this and does `cd`
            let path = worktree::resolve_path(&name)?;
            println!("{}", path.display());
        }
        Commands::Lock { name, reason } => {
            worktree::lock(&name, reason.as_deref())?;
        }
        Commands::Unlock { name } => {
            worktree::unlock(&name)?;
        }
        Commands::Move { name, new_path } => {
            worktree::move_worktree(&name, &new_path)?;
        }
        Commands::Repair { path } => {
            worktree::repair(path.as_deref())?;
        }
        Commands::Prune {
            dry_run,
            verbose,
            expire,
        } => {
            worktree::prune(dry_run, verbose, expire.as_deref())?;
        }
        Commands::Setup { shell } => setup(&shell)?,
        Commands::Init { shell } => print_init(&shell),
        Commands::Completions { shell } => print_completions(&shell),
        Commands::ListNames => {
            worktree::list_names()?;
        }
    }

    Ok(())
}

// ── Setup ────────────────────────────────────────────────────────────────────

fn setup(shell: &Shell) -> Result<()> {
    match shell {
        Shell::Zsh => setup_zsh(),
        Shell::Fish => setup_fish(),
        Shell::Bash => setup_bash(),
    }
}

fn write_file(path: &PathBuf, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }
    std::fs::write(path, content)
        .with_context(|| format!("Failed to write {}", path.display()))?;
    eprintln!("Wrote {}", path.display());
    Ok(())
}

fn home() -> PathBuf {
    dirs::home_dir().expect("Could not determine home directory")
}

fn setup_zsh() -> Result<()> {
    // The completion file goes in ~/.zfunc/_work, which zsh autoloads via $fpath.
    // The file content is the function body directly (traditional zsh autoload format),
    // so no _work(){} wrapper — the #compdef directive handles registration.
    let completion_path = home().join(".zfunc").join("_work");
    write_file(&completion_path, ZSH_FPATH_COMPLETION)?;

    println!();
    println!("Add the following to your ~/.zshrc (before compinit / oh-my-zsh source):");
    println!();
    println!("  fpath=(~/.zfunc $fpath)");
    println!();
    println!("And add this shell function so `work cd` can change your directory:");
    println!();
    println!("{}", ZSH_FUNCTION.trim_end().lines().map(|l| format!("  {l}")).collect::<Vec<_>>().join("\n"));
    println!();
    Ok(())
}

fn setup_fish() -> Result<()> {
    // Fish auto-loads from these two directories — no config file changes needed.
    let completions_path = home()
        .join(".config/fish/completions/work.fish");
    let function_path = home()
        .join(".config/fish/functions/work.fish");

    write_file(&completions_path, FISH_COMPLETION)?;
    write_file(&function_path, FISH_FUNCTION)?;

    println!();
    println!("Done! Open a new fish shell and `work <TAB>` should work.");
    println!();
    Ok(())
}

fn setup_bash() -> Result<()> {
    // bash-completion v2 auto-sources files from this directory.
    let completion_path = home()
        .join(".local/share/bash-completion/completions/work");
    write_file(&completion_path, BASH_COMPLETION)?;

    println!();
    println!("Add this shell function to your ~/.bashrc so `work cd` can change your directory:");
    println!();
    println!("{}", BASH_FUNCTION.trim_end().lines().map(|l| format!("  {l}")).collect::<Vec<_>>().join("\n"));
    println!();
    Ok(())
}

// ── Shell init (manual / eval alternative) ───────────────────────────────────

fn print_init(shell: &Shell) {
    match shell {
        Shell::Zsh => print!("{}", ZSH_FUNCTION),
        Shell::Fish => print!("{}", FISH_FUNCTION),
        Shell::Bash => print!("{}", BASH_FUNCTION),
    }
}

fn print_completions(shell: &Shell) {
    match shell {
        Shell::Zsh => print!("{}", ZSH_COMPLETION),
        Shell::Fish => print!("{}", FISH_COMPLETION),
        Shell::Bash => print!("{}", BASH_COMPLETION),
    }
}

// ── Shell functions (cd wrapper) ─────────────────────────────────────────────

const ZSH_FUNCTION: &str = r#"# work — git worktree manager (zsh function)
work() {
    if [[ "$1" == "cd" ]]; then
        local target
        target=$(command work cd "${@:2}") || return 1
        builtin cd "$target"
    else
        command work "$@"
    fi
}
"#;

const BASH_FUNCTION: &str = r#"# work — git worktree manager (bash function)
work() {
    if [[ "$1" == "cd" ]]; then
        local target
        target=$(command work cd "${@:2}") || return 1
        builtin cd "$target"
    else
        command work "$@"
    fi
}
"#;

const FISH_FUNCTION: &str = r#"# work — git worktree manager (fish function)
# ~/.config/fish/functions/work.fish
function work
    if test "$argv[1]" = "cd"
        set target (command work cd $argv[2..])
        and cd $target
    else
        command work $argv
    end
end
"#;

// ── Completion scripts ────────────────────────────────────────────────────────
//
// ZSH_FPATH_COMPLETION  — written to ~/.zfunc/_work by `work setup zsh`.
//   Zsh autoloads this as the body of the _work function (traditional format).
//   #compdef registers it; no explicit compdef call or _work(){} wrapper needed.
//
// ZSH_COMPLETION        — printed by `work completions zsh` for manual sourcing.
//   Wraps the logic in _work(){} and calls compdef explicitly.
//
// `work list-names` is called at completion time for dynamic worktree names.

const ZSH_FPATH_COMPLETION: &str = r#"#compdef work

_work_worktree_names() {
    local names
    names=(${(f)"$(command work list-names 2>/dev/null)"})
    _describe 'worktree' names
}

local state

_arguments -C \
    '(-h --help)'{-h,--help}'[Show help]' \
    '(-V --version)'{-V,--version}'[Show version]' \
    '1: :->command' \
    '*:: :->args'

case $state in
    command)
        local commands=(
            'add:Add a new worktree in ~/.worktrees/<repo>/<name>'
            'list:List worktrees'
            'remove:Remove a worktree'
            'cd:Change directory into a worktree'
            'lock:Lock a worktree'
            'unlock:Unlock a worktree'
            'move:Move a worktree to a new path'
            'repair:Repair worktree administrative files'
            'prune:Prune worktree information'
            'setup:Install completions to standard shell locations'
            'init:Print shell cd-wrapper function'
            'completions:Print shell completion script'
        )
        _describe 'command' commands
        ;;
    args)
        case $words[1] in
            add)
                _arguments \
                    '(-b --branch)'{-b,--branch}'[Create and checkout a new branch]:branch name:' \
                    '--detach[Detach HEAD]' \
                    '--no-checkout[Skip file checkout after creating the worktree]' \
                    '1:worktree name:' \
                    '2:commit-ish:'
                ;;
            list)
                _arguments \
                    '--porcelain[Porcelain output format]' \
                    '(-v --verbose)'{-v,--verbose}'[Verbose output]'
                ;;
            remove)
                _arguments \
                    '(-f --force)'{-f,--force}'[Force removal]' \
                    '1:worktree:_work_worktree_names'
                ;;
            cd)
                _arguments '1:worktree:_work_worktree_names'
                ;;
            lock)
                _arguments \
                    '--reason[Reason for locking]:reason:' \
                    '1:worktree:_work_worktree_names'
                ;;
            unlock)
                _arguments '1:worktree:_work_worktree_names'
                ;;
            move)
                _arguments \
                    '1:worktree:_work_worktree_names' \
                    '2:new path:_files -/'
                ;;
            repair)
                _arguments '1:path:_files -/'
                ;;
            prune)
                _arguments \
                    '(-n --dry-run)'{-n,--dry-run}'[Show what would be pruned]' \
                    '(-v --verbose)'{-v,--verbose}'[Report all removals]' \
                    '--expire[Only expire entries older than this]:expire:'
                ;;
            setup|init|completions)
                _arguments '1:shell:(zsh fish bash)'
                ;;
        esac
        ;;
esac
"#;

const ZSH_COMPLETION: &str = r#"#compdef work

_work_worktree_names() {
    local names
    names=(${(f)"$(command work list-names 2>/dev/null)"})
    _describe 'worktree' names
}

_work() {
    local state

    _arguments -C \
        '(-h --help)'{-h,--help}'[Show help]' \
        '(-V --version)'{-V,--version}'[Show version]' \
        '1: :->command' \
        '*:: :->args'

    case $state in
        command)
            local commands=(
                'add:Add a new worktree in ~/.worktrees/<repo>/<name>'
                'list:List worktrees'
                'remove:Remove a worktree'
                'cd:Change directory into a worktree'
                'lock:Lock a worktree'
                'unlock:Unlock a worktree'
                'move:Move a worktree to a new path'
                'repair:Repair worktree administrative files'
                'prune:Prune worktree information'
                'setup:Install completions to standard shell locations'
                'init:Print shell cd-wrapper function'
                'completions:Print shell completion script'
            )
            _describe 'command' commands
            ;;
        args)
            case $words[1] in
                add)
                    _arguments \
                        '(-b --branch)'{-b,--branch}'[Create and checkout a new branch]:branch name:' \
                        '--detach[Detach HEAD]' \
                        '--no-checkout[Skip file checkout after creating the worktree]' \
                        '1:worktree name:' \
                        '2:commit-ish:'
                    ;;
                list)
                    _arguments \
                        '--porcelain[Porcelain output format]' \
                        '(-v --verbose)'{-v,--verbose}'[Verbose output]'
                    ;;
                remove)
                    _arguments \
                        '(-f --force)'{-f,--force}'[Force removal]' \
                        '1:worktree:_work_worktree_names'
                    ;;
                cd)
                    _arguments '1:worktree:_work_worktree_names'
                    ;;
                lock)
                    _arguments \
                        '--reason[Reason for locking]:reason:' \
                        '1:worktree:_work_worktree_names'
                    ;;
                unlock)
                    _arguments '1:worktree:_work_worktree_names'
                    ;;
                move)
                    _arguments \
                        '1:worktree:_work_worktree_names' \
                        '2:new path:_files -/'
                    ;;
                repair)
                    _arguments '1:path:_files -/'
                    ;;
                prune)
                    _arguments \
                        '(-n --dry-run)'{-n,--dry-run}'[Show what would be pruned]' \
                        '(-v --verbose)'{-v,--verbose}'[Report all removals]' \
                        '--expire[Only expire entries older than this]:expire:'
                    ;;
                setup|init|completions)
                    _arguments '1:shell:(zsh fish bash)'
                    ;;
            esac
            ;;
    esac
}

compdef _work work
"#;

const FISH_COMPLETION: &str = r#"# work — git worktree manager (fish completions)
# ~/.config/fish/completions/work.fish

function __work_worktree_names
    command work list-names 2>/dev/null
end

set -l __work_cmds add list remove cd lock unlock move repair prune setup init completions

complete -c work -f

# Subcommands
complete -c work -n "not __fish_seen_subcommand_from $__work_cmds" -a add         -d 'Add a new worktree'
complete -c work -n "not __fish_seen_subcommand_from $__work_cmds" -a list        -d 'List worktrees'
complete -c work -n "not __fish_seen_subcommand_from $__work_cmds" -a remove      -d 'Remove a worktree'
complete -c work -n "not __fish_seen_subcommand_from $__work_cmds" -a cd          -d 'Change directory into a worktree'
complete -c work -n "not __fish_seen_subcommand_from $__work_cmds" -a lock        -d 'Lock a worktree'
complete -c work -n "not __fish_seen_subcommand_from $__work_cmds" -a unlock      -d 'Unlock a worktree'
complete -c work -n "not __fish_seen_subcommand_from $__work_cmds" -a move        -d 'Move a worktree to a new path'
complete -c work -n "not __fish_seen_subcommand_from $__work_cmds" -a repair      -d 'Repair worktree administrative files'
complete -c work -n "not __fish_seen_subcommand_from $__work_cmds" -a prune       -d 'Prune worktree information'
complete -c work -n "not __fish_seen_subcommand_from $__work_cmds" -a setup       -d 'Install completions to standard shell locations'
complete -c work -n "not __fish_seen_subcommand_from $__work_cmds" -a init        -d 'Print shell cd-wrapper function'
complete -c work -n "not __fish_seen_subcommand_from $__work_cmds" -a completions -d 'Print shell completion script'

# add
complete -c work -n "__fish_seen_subcommand_from add" -s b -l branch       -d 'Create and checkout a new branch' -r
complete -c work -n "__fish_seen_subcommand_from add" -l detach             -d 'Detach HEAD'
complete -c work -n "__fish_seen_subcommand_from add" -l no-checkout        -d 'Skip file checkout'

# list
complete -c work -n "__fish_seen_subcommand_from list" -l porcelain         -d 'Porcelain output format'
complete -c work -n "__fish_seen_subcommand_from list" -s v -l verbose      -d 'Verbose output'

# remove
complete -c work -n "__fish_seen_subcommand_from remove" -s f -l force      -d 'Force removal'
complete -c work -n "__fish_seen_subcommand_from remove" -a "(__work_worktree_names)"

# cd
complete -c work -n "__fish_seen_subcommand_from cd" -a "(__work_worktree_names)"

# lock
complete -c work -n "__fish_seen_subcommand_from lock" -l reason            -d 'Reason for locking' -r
complete -c work -n "__fish_seen_subcommand_from lock" -a "(__work_worktree_names)"

# unlock
complete -c work -n "__fish_seen_subcommand_from unlock" -a "(__work_worktree_names)"

# move
complete -c work -n "__fish_seen_subcommand_from move" -a "(__work_worktree_names)"

# prune
complete -c work -n "__fish_seen_subcommand_from prune" -s n -l dry-run     -d 'Show what would be pruned'
complete -c work -n "__fish_seen_subcommand_from prune" -s v -l verbose     -d 'Report all removals'
complete -c work -n "__fish_seen_subcommand_from prune" -l expire           -d 'Only expire entries older than this' -r

# setup / init / completions — shell names
complete -c work -n "__fish_seen_subcommand_from setup init completions" -a "zsh fish bash"
"#;

const BASH_COMPLETION: &str = r#"# work — git worktree manager (bash completions)
# ~/.local/share/bash-completion/completions/work
_work() {
    local cur prev words cword
    _init_completion || return

    local all_commands="add list remove cd lock unlock move repair prune setup init completions"

    if [[ $cword -eq 1 ]]; then
        COMPREPLY=($(compgen -W "$all_commands" -- "$cur"))
        return
    fi

    local subcmd="${words[1]}"

    case "$subcmd" in
        remove|cd|lock|unlock|move)
            local names
            names=$(command work list-names 2>/dev/null)
            COMPREPLY=($(compgen -W "$names" -- "$cur"))
            ;;
        add)
            case "$prev" in
                -b|--branch|--expire) return ;;
            esac
            COMPREPLY=($(compgen -W "--branch --detach --no-checkout" -- "$cur"))
            ;;
        list)
            COMPREPLY=($(compgen -W "--porcelain --verbose" -- "$cur"))
            ;;
        prune)
            case "$prev" in
                --expire) return ;;
            esac
            COMPREPLY=($(compgen -W "--dry-run --verbose --expire" -- "$cur"))
            ;;
        setup|init|completions)
            COMPREPLY=($(compgen -W "zsh fish bash" -- "$cur"))
            ;;
    esac
}

complete -F _work work
"#;
