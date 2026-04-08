use anyhow::{anyhow, Context, Result};
use std::path::PathBuf;
use std::process::Command;

// ── VCS detection ─────────────────────────────────────────────────────────────

/// Returns true when the current directory is inside a jj workspace.
/// Prefers jj over git in colocated repos, matching a jj-first workflow.
pub fn detect_jj() -> bool {
    Command::new("jj")
        .args(["workspace", "root"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn get_repo_name() -> Result<String> {
    // Try git remote get-url origin first
    let output = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output()
        .context("Failed to run git")?;

    if output.status.success() {
        let url = String::from_utf8(output.stdout)
            .unwrap_or_default()
            .trim()
            .to_string();
        let name = url
            .split('/')
            .last()
            .unwrap_or("")
            .trim_end_matches(".git")
            .to_string();
        if !name.is_empty() {
            return Ok(name);
        }
    }

    // Fall back to the toplevel directory name
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .context("Failed to run git rev-parse")?;

    if !output.status.success() {
        return Err(anyhow!("Not in a git repository"));
    }

    let root = String::from_utf8(output.stdout)
        .unwrap_or_default()
        .trim()
        .to_string();

    let name = PathBuf::from(&root)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    Ok(name)
}

pub fn worktrees_base() -> PathBuf {
    dirs::home_dir()
        .expect("Could not determine home directory")
        .join(".worktrees")
}

pub fn repo_worktrees_dir() -> Result<PathBuf> {
    Ok(worktrees_base().join(get_repo_name()?))
}

pub fn resolve_path(name: &str) -> Result<PathBuf> {
    if detect_jj() {
        return crate::jj::resolve_path(name);
    }
    let path = repo_worktrees_dir()?.join(name);
    if !path.exists() {
        return Err(anyhow!(
            "Worktree '{}' not found at {}",
            name,
            path.display()
        ));
    }
    Ok(path)
}

pub fn add(
    name: &str,
    commit_ish: Option<&str>,
    branch: Option<&str>,
    detach: bool,
    no_checkout: bool,
) -> Result<()> {
    if detect_jj() {
        if branch.is_some() || detach || no_checkout {
            eprintln!("note: --branch, --detach, and --no-checkout are git-only; ignored in jj mode");
        }
        return crate::jj::add(name, commit_ish);
    }
    let base = repo_worktrees_dir()?;
    std::fs::create_dir_all(&base)
        .with_context(|| format!("Failed to create directory {}", base.display()))?;

    let path = base.join(name);

    let mut cmd = Command::new("git");
    cmd.args(["worktree", "add"]);

    if detach {
        cmd.arg("--detach");
    } else if let Some(b) = branch {
        cmd.arg("-b").arg(b);
    }

    if no_checkout {
        cmd.arg("--no-checkout");
    }

    cmd.arg(&path);

    if let Some(c) = commit_ish {
        cmd.arg(c);
    }

    // Use output() instead of status() so git's stdout doesn't leak into our stdout.
    // The shell function captures our stdout to get the worktree path; any git
    // progress messages mixed in would corrupt it and cause `cd` to fail.
    let output = cmd.output().context("Failed to run git worktree add")?;
    std::io::Write::write_all(&mut std::io::stderr(), &output.stdout).ok();
    std::io::Write::write_all(&mut std::io::stderr(), &output.stderr).ok();
    if !output.status.success() {
        return Err(anyhow!("git worktree add failed"));
    }

    eprintln!("Worktree '{}' created at {}", name, path.display());
    Ok(())
}

pub fn list(porcelain: bool, verbose: bool) -> Result<()> {
    if detect_jj() {
        if porcelain || verbose {
            eprintln!("note: --porcelain and --verbose are git-only; running plain jj workspace list");
        }
        return crate::jj::list();
    }
    let mut cmd = Command::new("git");
    cmd.args(["worktree", "list"]);
    if porcelain {
        cmd.arg("--porcelain");
    }
    if verbose {
        cmd.arg("-v");
    }
    let status = cmd.status().context("Failed to run git worktree list")?;
    if !status.success() {
        return Err(anyhow!("git worktree list failed"));
    }
    Ok(())
}

pub fn remove(name: &str, force: bool) -> Result<()> {
    if detect_jj() {
        if force {
            eprintln!("note: --force is a git-only flag; jj workspace forget always proceeds");
        }
        return crate::jj::remove(name);
    }
    let path = resolve_path(name)?;
    let mut cmd = Command::new("git");
    cmd.args(["worktree", "remove"]);
    if force {
        cmd.arg("--force");
    }
    cmd.arg(&path);
    let status = cmd.status().context("Failed to run git worktree remove")?;
    if !status.success() {
        return Err(anyhow!("git worktree remove failed"));
    }
    Ok(())
}

pub fn lock(name: &str, reason: Option<&str>) -> Result<()> {
    if detect_jj() {
        return Err(anyhow!("'work lock' is not supported in jj mode (jj workspaces have no lock mechanism)"));
    }
    let path = resolve_path(name)?;
    let mut cmd = Command::new("git");
    cmd.args(["worktree", "lock"]);
    if let Some(r) = reason {
        cmd.arg("--reason").arg(r);
    }
    cmd.arg(&path);
    let status = cmd.status().context("Failed to run git worktree lock")?;
    if !status.success() {
        return Err(anyhow!("git worktree lock failed"));
    }
    Ok(())
}

pub fn unlock(name: &str) -> Result<()> {
    if detect_jj() {
        return Err(anyhow!("'work unlock' is not supported in jj mode"));
    }
    let path = resolve_path(name)?;
    let status = Command::new("git")
        .args(["worktree", "unlock"])
        .arg(&path)
        .status()
        .context("Failed to run git worktree unlock")?;
    if !status.success() {
        return Err(anyhow!("git worktree unlock failed"));
    }
    Ok(())
}

pub fn move_worktree(name: &str, new_path: &str) -> Result<()> {
    if detect_jj() {
        return crate::jj::move_workspace(name, new_path);
    }
    let path = resolve_path(name)?;
    let status = Command::new("git")
        .args(["worktree", "move"])
        .arg(&path)
        .arg(new_path)
        .status()
        .context("Failed to run git worktree move")?;
    if !status.success() {
        return Err(anyhow!("git worktree move failed"));
    }
    Ok(())
}

pub fn repair(path: Option<&str>) -> Result<()> {
    if detect_jj() {
        return Err(anyhow!("'work repair' is not supported in jj mode (jj workspaces self-heal)"));
    }
    let mut cmd = Command::new("git");
    cmd.args(["worktree", "repair"]);
    if let Some(p) = path {
        cmd.arg(p);
    }
    let status = cmd.status().context("Failed to run git worktree repair")?;
    if !status.success() {
        return Err(anyhow!("git worktree repair failed"));
    }
    Ok(())
}

pub fn prune(dry_run: bool, verbose: bool, expire: Option<&str>) -> Result<()> {
    if detect_jj() {
        return Err(anyhow!("'work prune' is not supported in jj mode (use 'work remove' to clean up individual workspaces)"));
    }
    let mut cmd = Command::new("git");
    cmd.args(["worktree", "prune"]);
    if dry_run {
        cmd.arg("-n");
    }
    if verbose {
        cmd.arg("-v");
    }
    if let Some(e) = expire {
        cmd.arg("--expire").arg(e);
    }
    let status = cmd.status().context("Failed to run git worktree prune")?;
    if !status.success() {
        return Err(anyhow!("git worktree prune failed"));
    }
    Ok(())
}

pub fn on(name: &str) -> Result<PathBuf> {
    if detect_jj() {
        return crate::jj::on(name);
    }
    let path = repo_worktrees_dir()?.join(name);
    if !path.exists() {
        add(name, None, None, false, false)?;
    }
    Ok(path)
}

pub fn list_names() -> Result<()> {
    if detect_jj() {
        return crate::jj::list_names();
    }
    let dir = match repo_worktrees_dir() {
        Ok(d) => d,
        Err(_) => return Ok(()),
    };

    if !dir.exists() {
        return Ok(());
    }

    let mut entries = std::fs::read_dir(&dir)
        .with_context(|| format!("Failed to read {}", dir.display()))?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect::<Vec<_>>();

    entries.sort();
    for name in entries {
        println!("{}", name);
    }

    Ok(())
}

fn worktree_branch(path: &std::path::Path) -> String {
    Command::new("git")
        .args(["-C", &path.to_string_lossy(), "branch", "--show-current"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

fn worktree_dirty(path: &std::path::Path) -> bool {
    Command::new("git")
        .args(["-C", &path.to_string_lossy(), "status", "--porcelain"])
        .output()
        .map(|o| !o.stdout.is_empty())
        .unwrap_or(false)
}


fn worktree_last_commit(path: &std::path::Path) -> String {
    Command::new("git")
        .args(["-C", &path.to_string_lossy(), "log", "-1", "--format=%s (%cr)"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

pub fn list_names_with_info() -> Result<()> {
    if detect_jj() {
        return crate::jj::list_names_with_info();
    }
    let dir = match repo_worktrees_dir() {
        Ok(d) => d,
        Err(_) => return Ok(()),
    };

    if !dir.exists() {
        return Ok(());
    }

    let mut entries = std::fs::read_dir(&dir)
        .with_context(|| format!("Failed to read {}", dir.display()))?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect::<Vec<_>>();

    entries.sort();
    for name in entries {
        let path = dir.join(&name);
        let branch = worktree_branch(&path);
        let dirty = worktree_dirty(&path);
        let commit = worktree_last_commit(&path);
        print_info_line(&name, None, &branch, dirty, &commit);
    }

    Ok(())
}

// ANSI helpers — used in fzf output (fzf renders raw ANSI with --ansi)
const RS: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const RED: &str = "\x1b[31m";
const CYAN: &str = "\x1b[36m";

pub fn fzf_display(name: &str, repo: Option<&str>, branch: &str, dirty: bool, commit: &str) -> String {
    let repo_part = match repo {
        Some(r) => format!("{DIM}[{r}]{RS} "),
        None => String::new(),
    };
    let dirty_part = if dirty { format!(" {RED}!{RS}") } else { String::new() };
    let commit_part = if commit.is_empty() {
        String::new()
    } else {
        format!(" {DIM}· {commit}{RS}")
    };
    format!(
        "{BOLD}{name}{RS}  {repo_part}{CYAN}{BOLD}{branch}{RS}{dirty_part}{commit_part}"
    )
}

pub fn list_fzf() -> Result<()> {
    if detect_jj() {
        return crate::jj::list_fzf();
    }
    // Try current repo first; fall back to all repos.
    let (dir, global) = match repo_worktrees_dir() {
        Ok(d) if d.exists() => (d, false),
        _ => {
            let base = worktrees_base();
            if !base.exists() { return Ok(()); }
            (base, true)
        }
    };

    if global {
        let mut rows: Vec<(String, String, String, bool, String)> = vec![];
        for repo_entry in std::fs::read_dir(&dir)? {
            let repo_entry = repo_entry?;
            if !repo_entry.file_type()?.is_dir() { continue; }
            let repo_name = repo_entry.file_name().to_string_lossy().to_string();
            for wt_entry in std::fs::read_dir(repo_entry.path())?.flatten() {
                if !wt_entry.file_type().map(|t| t.is_dir()).unwrap_or(false) { continue; }
                let name = wt_entry.file_name().to_string_lossy().to_string();
                let path = wt_entry.path();
                rows.push((name, repo_name.clone(), worktree_branch(&path), worktree_dirty(&path), worktree_last_commit(&path)));
            }
        }
        rows.sort_by(|a, b| a.0.cmp(&b.0));
        for (name, repo, branch, dirty, commit) in rows {
            println!("{}\t{}", name, fzf_display(&name, Some(&repo), &branch, dirty, &commit));
        }
    } else {
        let mut entries: Vec<String> = std::fs::read_dir(&dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        entries.sort();
        for name in entries {
            let path = dir.join(&name);
            println!("{}\t{}", name, fzf_display(&name, None, &worktree_branch(&path), worktree_dirty(&path), &worktree_last_commit(&path)));
        }
    }
    Ok(())
}

// Output format for completion commands: tab-separated fields with no ANSI codes.
// Fields: branch \t dirty(1|0) \t commit \t repo(optional)
// Zsh applies its own %F{} formatting so it can correctly calculate visual width.
pub fn print_info_line(name: &str, repo: Option<&str>, branch: &str, dirty: bool, commit: &str) {
    let dirty_flag = if dirty { "1" } else { "0" };
    let repo_field = repo.unwrap_or("");
    println!("{}\t{}\t{}\t{}\t{}", name, branch, dirty_flag, commit, repo_field);
}

pub fn list_all_names_with_info() -> Result<()> {
    if detect_jj() {
        return crate::jj::list_all_names_with_info();
    }
    let base = worktrees_base();
    if !base.exists() {
        return Ok(());
    }

    let mut rows: Vec<(String, String, String, bool, String)> = vec![];

    for repo_entry in std::fs::read_dir(&base)
        .with_context(|| format!("Failed to read {}", base.display()))?
    {
        let repo_entry = repo_entry?;
        if !repo_entry.file_type()?.is_dir() {
            continue;
        }
        let repo_name = repo_entry.file_name().to_string_lossy().to_string();
        if let Ok(wt_entries) = std::fs::read_dir(repo_entry.path()) {
            for wt_entry in wt_entries.flatten() {
                if !wt_entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    continue;
                }
                let name = wt_entry.file_name().to_string_lossy().to_string();
                let path = wt_entry.path();
                let branch = worktree_branch(&path);
                let dirty = worktree_dirty(&path);
                let commit = worktree_last_commit(&path);
                rows.push((name, repo_name.clone(), branch, dirty, commit));
            }
        }
    }

    rows.sort_by(|a, b| a.0.cmp(&b.0));
    for (name, repo, branch, dirty, commit) in rows {
        print_info_line(&name, Some(&repo), &branch, dirty, &commit);
    }

    Ok(())
}

pub fn list_all_names() -> Result<()> {
    if detect_jj() {
        return crate::jj::list_all_names();
    }
    let base = worktrees_base();
    if !base.exists() {
        return Ok(());
    }

    let mut names = std::collections::HashSet::new();

    for repo_entry in std::fs::read_dir(&base)
        .with_context(|| format!("Failed to read {}", base.display()))?
    {
        let repo_entry = repo_entry?;
        if !repo_entry.file_type()?.is_dir() {
            continue;
        }
        if let Ok(wt_entries) = std::fs::read_dir(repo_entry.path()) {
            for wt_entry in wt_entries.flatten() {
                if wt_entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    names.insert(wt_entry.file_name().to_string_lossy().to_string());
                }
            }
        }
    }

    let mut names: Vec<_> = names.into_iter().collect();
    names.sort();
    for name in names {
        println!("{}", name);
    }

    Ok(())
}
