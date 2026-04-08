use anyhow::{anyhow, Context, Result};
use std::path::PathBuf;
use std::process::Command;

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

    let status = cmd.status().context("Failed to run git worktree add")?;
    if !status.success() {
        return Err(anyhow!("git worktree add failed"));
    }

    eprintln!("Worktree '{}' created at {}", name, path.display());
    Ok(())
}

pub fn list(porcelain: bool, verbose: bool) -> Result<()> {
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
    let path = repo_worktrees_dir()?.join(name);
    if !path.exists() {
        add(name, None, None, false, false)?;
    }
    Ok(path)
}

pub fn list_names() -> Result<()> {
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

pub fn list_all_names() -> Result<()> {
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
