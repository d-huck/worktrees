use anyhow::{anyhow, Context, Result};
use std::path::PathBuf;
use std::process::Command;

use crate::worktree::{fzf_display, print_info_line, worktrees_base};

// ── Repo / path helpers ───────────────────────────────────────────────────────

pub fn get_repo_name() -> Result<String> {
    // Try jj git remote list for an "origin" URL.
    let output = Command::new("jj")
        .args(["git", "remote", "list"])
        .stderr(std::process::Stdio::null())
        .output()
        .context("Failed to run jj")?;

    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines() {
            // Output is "name URL" (space-separated)
            let mut parts = line.splitn(2, char::is_whitespace);
            if let (Some(name), Some(url)) = (parts.next(), parts.next()) {
                if name == "origin" {
                    let repo_name = url
                        .trim()
                        .split('/')
                        .last()
                        .unwrap_or("")
                        .trim_end_matches(".git")
                        .to_string();
                    if !repo_name.is_empty() {
                        return Ok(repo_name);
                    }
                }
            }
        }
    }

    // Fall back to the workspace root directory name.
    let output = Command::new("jj")
        .args(["workspace", "root"])
        .stderr(std::process::Stdio::null())
        .output()
        .context("Failed to run jj workspace root")?;

    if output.status.success() {
        let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let name = PathBuf::from(&root)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        return Ok(name);
    }

    Err(anyhow!("Not in a jj repository"))
}

pub fn repo_workspaces_dir() -> Result<PathBuf> {
    Ok(worktrees_base().join(get_repo_name()?))
}

/// The jj repo root (shared across all workspaces for this repo).
fn jj_root() -> Result<String> {
    let output = Command::new("jj")
        .args(["workspace", "root"])
        .stderr(std::process::Stdio::null())
        .output()
        .context("Failed to run jj workspace root")?;
    if !output.status.success() {
        return Err(anyhow!("Not in a jj workspace"));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn resolve_path(name: &str) -> Result<PathBuf> {
    let path = repo_workspaces_dir()?.join(name);
    if !path.exists() {
        return Err(anyhow!(
            "Workspace '{}' not found at {}",
            name,
            path.display()
        ));
    }
    Ok(path)
}

// ── Core workspace operations ─────────────────────────────────────────────────

pub fn add(name: &str, revision: Option<&str>) -> Result<()> {
    let base = repo_workspaces_dir()?;
    std::fs::create_dir_all(&base)
        .with_context(|| format!("Failed to create directory {}", base.display()))?;

    let path = base.join(name);

    let mut cmd = Command::new("jj");
    cmd.args(["workspace", "add"]);
    cmd.arg(&path);
    cmd.arg("--name").arg(name);

    if let Some(rev) = revision {
        cmd.arg("--revision").arg(rev);
    }

    // Capture stdout so it doesn't corrupt the path printed by `on`.
    let output = cmd.output().context("Failed to run jj workspace add")?;
    std::io::Write::write_all(&mut std::io::stderr(), &output.stdout).ok();
    std::io::Write::write_all(&mut std::io::stderr(), &output.stderr).ok();

    if !output.status.success() {
        return Err(anyhow!("jj workspace add failed"));
    }

    eprintln!("Workspace '{}' created at {}", name, path.display());
    Ok(())
}

pub fn list() -> Result<()> {
    let status = Command::new("jj")
        .args(["workspace", "list"])
        .status()
        .context("Failed to run jj workspace list")?;
    if !status.success() {
        return Err(anyhow!("jj workspace list failed"));
    }
    Ok(())
}

pub fn remove(name: &str) -> Result<()> {
    let path = resolve_path(name)?;

    let output = Command::new("jj")
        .args(["workspace", "forget", name])
        .output()
        .context("Failed to run jj workspace forget")?;

    std::io::Write::write_all(&mut std::io::stderr(), &output.stderr).ok();

    if !output.status.success() {
        return Err(anyhow!("jj workspace forget failed"));
    }

    std::fs::remove_dir_all(&path)
        .with_context(|| format!("Failed to remove directory {}", path.display()))?;

    eprintln!("Workspace '{}' removed", name);
    Ok(())
}

/// Move a managed workspace to a new path by forgetting it, moving the directory,
/// and re-registering it with `jj workspace add --name <name>`.
pub fn move_workspace(name: &str, new_path: &str) -> Result<()> {
    let old_path = resolve_path(name)?;
    let new_path_buf = PathBuf::from(new_path);

    // Forget the workspace first (while the old path still exists).
    let output = Command::new("jj")
        .args(["workspace", "forget", name])
        .output()
        .context("Failed to run jj workspace forget")?;
    std::io::Write::write_all(&mut std::io::stderr(), &output.stderr).ok();
    if !output.status.success() {
        return Err(anyhow!("jj workspace forget failed"));
    }

    // Move the directory.
    if let Some(parent) = new_path_buf.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }
    std::fs::rename(&old_path, &new_path_buf)
        .with_context(|| format!("Failed to move {} → {}", old_path.display(), new_path_buf.display()))?;

    // Re-register the workspace at its new location.
    let output = Command::new("jj")
        .args(["workspace", "add"])
        .arg(&new_path_buf)
        .arg("--name")
        .arg(name)
        .output()
        .context("Failed to run jj workspace add")?;
    std::io::Write::write_all(&mut std::io::stderr(), &output.stdout).ok();
    std::io::Write::write_all(&mut std::io::stderr(), &output.stderr).ok();
    if !output.status.success() {
        return Err(anyhow!("jj workspace add failed after move"));
    }

    eprintln!("Workspace '{}' moved to {}", name, new_path_buf.display());
    Ok(())
}

pub fn on(name: &str) -> Result<PathBuf> {
    let path = repo_workspaces_dir()?.join(name);
    if !path.exists() {
        add(name, None)?;
    }
    Ok(path)
}

// ── Per-workspace info (used by completion / fzf) ────────────────────────────

/// Bookmarks at a workspace's working-copy commit — displayed like "branch" in git mode.
fn workspace_bookmarks(root: &str, workspace_name: &str) -> String {
    Command::new("jj")
        .args([
            "-R",
            root,
            "--no-pager",
            "log",
            "-r",
            &format!("workspace_head('{workspace_name}')"),
            "-T",
            "separate(' ', bookmarks)",
            "--no-graph",
        ])
        .stderr(std::process::Stdio::null())
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

/// First line of the description at a workspace's working-copy commit.
fn workspace_description(root: &str, workspace_name: &str) -> String {
    Command::new("jj")
        .args([
            "-R",
            root,
            "--no-pager",
            "log",
            "-r",
            &format!("workspace_head('{workspace_name}')"),
            "-T",
            r#"if(description, description.first_line(), "(no description)")"#,
            "--no-graph",
        ])
        .stderr(std::process::Stdio::null())
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

/// Returns true when the working-copy commit is non-empty (has changes vs its parent).
fn workspace_has_changes(root: &str, workspace_name: &str) -> bool {
    Command::new("jj")
        .args([
            "-R",
            root,
            "--no-pager",
            "log",
            "-r",
            &format!("workspace_head('{workspace_name}')"),
            "-T",
            "if(empty, '0', '1')",
            "--no-graph",
        ])
        .stderr(std::process::Stdio::null())
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "1")
        .unwrap_or(false)
}

// ── Listing / completion helpers ──────────────────────────────────────────────

fn read_managed_workspace_names(dir: &std::path::Path) -> Result<Vec<String>> {
    let mut entries = std::fs::read_dir(dir)
        .with_context(|| format!("Failed to read {}", dir.display()))?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect::<Vec<_>>();
    entries.sort();
    Ok(entries)
}

pub fn list_names() -> Result<()> {
    let dir = match repo_workspaces_dir() {
        Ok(d) => d,
        Err(_) => return Ok(()),
    };
    if !dir.exists() {
        return Ok(());
    }
    for name in read_managed_workspace_names(&dir)? {
        println!("{}", name);
    }
    Ok(())
}

pub fn list_names_with_info() -> Result<()> {
    let dir = match repo_workspaces_dir() {
        Ok(d) => d,
        Err(_) => return Ok(()),
    };
    if !dir.exists() {
        return Ok(());
    }
    let root = jj_root().unwrap_or_default();
    for name in read_managed_workspace_names(&dir)? {
        let bookmarks = workspace_bookmarks(&root, &name);
        let has_changes = workspace_has_changes(&root, &name);
        let description = workspace_description(&root, &name);
        print_info_line(&name, None, &bookmarks, has_changes, &description);
    }
    Ok(())
}

pub fn list_fzf() -> Result<()> {
    let (dir, global) = match repo_workspaces_dir() {
        Ok(d) if d.exists() => (d, false),
        _ => {
            let base = worktrees_base();
            if !base.exists() {
                return Ok(());
            }
            (base, true)
        }
    };

    let root = jj_root().unwrap_or_default();

    if global {
        let mut rows: Vec<(String, String, String, bool, String)> = vec![];
        for repo_entry in std::fs::read_dir(&dir)? {
            let repo_entry = repo_entry?;
            if !repo_entry.file_type()?.is_dir() {
                continue;
            }
            let repo_name = repo_entry.file_name().to_string_lossy().to_string();
            for wt_entry in std::fs::read_dir(repo_entry.path())?.flatten() {
                if !wt_entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    continue;
                }
                let name = wt_entry.file_name().to_string_lossy().to_string();
                rows.push((
                    name.clone(),
                    repo_name.clone(),
                    workspace_bookmarks(&root, &name),
                    workspace_has_changes(&root, &name),
                    workspace_description(&root, &name),
                ));
            }
        }
        rows.sort_by(|a, b| a.0.cmp(&b.0));
        for (name, repo, bookmarks, has_changes, desc) in rows {
            println!(
                "{}\t{}",
                name,
                fzf_display(&name, Some(&repo), &bookmarks, has_changes, &desc)
            );
        }
    } else {
        let entries = read_managed_workspace_names(&dir)?;
        for name in entries {
            let bookmarks = workspace_bookmarks(&root, &name);
            let has_changes = workspace_has_changes(&root, &name);
            let desc = workspace_description(&root, &name);
            println!("{}\t{}", name, fzf_display(&name, None, &bookmarks, has_changes, &desc));
        }
    }
    Ok(())
}

pub fn list_all_names_with_info() -> Result<()> {
    let base = worktrees_base();
    if !base.exists() {
        return Ok(());
    }
    let root = jj_root().unwrap_or_default();
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
                rows.push((
                    name.clone(),
                    repo_name.clone(),
                    workspace_bookmarks(&root, &name),
                    workspace_has_changes(&root, &name),
                    workspace_description(&root, &name),
                ));
            }
        }
    }

    rows.sort_by(|a, b| a.0.cmp(&b.0));
    for (name, repo, bookmarks, has_changes, desc) in rows {
        print_info_line(&name, Some(&repo), &bookmarks, has_changes, &desc);
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
