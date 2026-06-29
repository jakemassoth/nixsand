use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};

use super::{GitBackend, WindowInfo, ZmxBackend};

// ---------------------------------------------------------------------------
// GitBackend — wraps `git`
// ---------------------------------------------------------------------------

pub struct RealGitBackend;

impl GitBackend for RealGitBackend {
    fn clone_bare(&self, url: &str, dest: &Path) -> Result<()> {
        let status = Command::new("git")
            .args(["clone", "--bare", url])
            .arg(dest)
            .status()
            .context("failed to run 'git clone --bare'")?;
        if !status.success() {
            bail!("git clone --bare failed for '{url}'");
        }
        Ok(())
    }

    fn set_config(&self, repo: &Path, key: &str, value: &str) -> Result<()> {
        let status = Command::new("git")
            .args(["-C"])
            .arg(repo)
            .args(["config", key, value])
            .status()
            .context("failed to run 'git config'")?;
        if !status.success() {
            bail!("git config {key} {value} failed");
        }
        Ok(())
    }

    fn unset_config(&self, repo: &Path, key: &str) -> Result<()> {
        // `git config --unset` exits 5 when the key is missing — treat that as success.
        let output = Command::new("git")
            .args(["-C"])
            .arg(repo)
            .args(["config", "--unset", key])
            .output()
            .context("failed to run 'git config --unset'")?;
        if output.status.success() {
            return Ok(());
        }
        if output.status.code() == Some(5) {
            return Ok(());
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git config --unset {} failed: {}", key, stderr.trim());
    }

    fn add_worktree(
        &self,
        bare_repo: &Path,
        worktree_path: &Path,
        branch: &str,
        base: &str,
    ) -> Result<()> {
        // Check whether the branch already exists in the repo.
        // If it does, check it out directly; if not, create it from base.
        let branch_exists = Command::new("git")
            .args(["-C"])
            .arg(bare_repo)
            .args(["rev-parse", "--verify", branch])
            .output()
            .is_ok_and(|o| o.status.success());

        let mut cmd = Command::new("git");
        cmd.args(["-C"]).arg(bare_repo).arg("worktree").arg("add");
        if branch_exists {
            cmd.arg(worktree_path).arg(branch);
        } else {
            cmd.args(["-b", branch]).arg(worktree_path).arg(base);
        }

        let status = cmd
            .status()
            .context("failed to run 'git worktree add'")?;
        if !status.success() {
            bail!("git worktree add failed for branch '{branch}' from '{base}'");
        }
        Ok(())
    }

    fn remove_worktree(&self, bare_repo: &Path, worktree_path: &Path) -> Result<()> {
        // `git worktree remove --force` drops the worktree and its admin files.
        // Fall back to pruning if the directory is already gone.
        let status = Command::new("git")
            .args(["-C"])
            .arg(bare_repo)
            .args(["worktree", "remove", "--force"])
            .arg(worktree_path)
            .status()
            .context("failed to run 'git worktree remove'")?;
        if !status.success() {
            // Best-effort: prune stale worktree metadata so the registry recovers.
            let _ = Command::new("git")
                .args(["-C"])
                .arg(bare_repo)
                .args(["worktree", "prune"])
                .status();
        }
        Ok(())
    }

    fn default_branch(&self, bare_repo: &Path) -> Result<String> {
        let output = Command::new("git")
            .args(["-C"])
            .arg(bare_repo)
            .args(["symbolic-ref", "--short", "HEAD"])
            .output()
            .context("failed to run 'git symbolic-ref'")?;
        if !output.status.success() {
            // Fall back to checking remote/HEAD
            let output2 = Command::new("git")
                .args(["-C"])
                .arg(bare_repo)
                .args(["remote", "show", "origin"])
                .output()
                .context("failed to run 'git remote show origin'")?;
            let text = String::from_utf8_lossy(&output2.stdout);
            for line in text.lines() {
                if line.trim().starts_with("HEAD branch:") {
                    let branch = line.split(':').nth(1).unwrap_or("main").trim().to_string();
                    return Ok(branch);
                }
            }
            return Ok("main".to_string());
        }
        let branch = String::from_utf8(output.stdout)
            .context("invalid UTF-8 in git output")?
            .trim()
            .to_string();
        if branch.is_empty() {
            Ok("main".to_string())
        } else {
            Ok(branch)
        }
    }
}

// ---------------------------------------------------------------------------
// ZmxBackend — wraps `tmux`
// ---------------------------------------------------------------------------

pub struct RealZmxBackend;

/// Build a `session:window` target string for tmux `-t` args.
fn target(session: &str, window: &str) -> String {
    format!("{session}:{window}")
}

impl ZmxBackend for RealZmxBackend {
    fn session_exists(&self, session: &str) -> Result<bool> {
        let output = Command::new("tmux")
            .args(["has-session", "-t", session])
            .output()
            .context("failed to run 'tmux has-session'")?;
        Ok(output.status.success())
    }

    fn ensure_session(&self, session: &str) -> Result<()> {
        if self.session_exists(session)? {
            return Ok(());
        }
        // Create a detached session. The initial window is a throwaway shell;
        // task windows are added with `new_window`.
        let status = Command::new("tmux")
            .args(["new-session", "-d", "-s", session, "-n", "nixsand"])
            .status()
            .context("failed to run 'tmux new-session'")?;
        if !status.success() {
            bail!("tmux new-session failed for '{session}'");
        }
        Ok(())
    }

    fn new_window(&self, session: &str, window: &str, cwd: &Path, command: &str) -> Result<()> {
        // Target the session with a trailing colon (`<session>:`) so tmux picks
        // the next free window index. A bare `-t <session>` target instead
        // tries to (re)use the session's base index, which fails with
        // "index N in use" under `base-index`/`renumber-windows` configs.
        let target = format!("{session}:");
        let mut cmd = Command::new("tmux");
        cmd.args(["new-window", "-t", &target, "-n", window, "-c"])
            .arg(cwd)
            // `--` terminates option parsing; the command runs via the user's shell.
            .args(["--", "sh", "-lc", command]);
        let status = cmd
            .status()
            .context("failed to run 'tmux new-window'")?;
        if !status.success() {
            bail!("tmux new-window failed for '{session}:{window}'");
        }
        Ok(())
    }

    fn window_exists(&self, session: &str, window: &str) -> Result<bool> {
        Ok(self.list_windows(session)?.iter().any(|w| w.name == window))
    }

    fn send_keys(&self, session: &str, window: &str, text: &str) -> Result<()> {
        let tgt = target(session, window);
        // Send the literal text (`-l`), then Enter as a separate key event.
        let status = Command::new("tmux")
            .args(["send-keys", "-t", &tgt, "-l", "--", text])
            .status()
            .context("failed to run 'tmux send-keys'")?;
        if !status.success() {
            bail!("tmux send-keys failed for '{tgt}'");
        }
        let status = Command::new("tmux")
            .args(["send-keys", "-t", &tgt, "Enter"])
            .status()
            .context("failed to run 'tmux send-keys Enter'")?;
        if !status.success() {
            bail!("tmux send-keys Enter failed for '{tgt}'");
        }
        Ok(())
    }

    fn capture_pane(&self, session: &str, window: &str, lines: Option<usize>) -> Result<String> {
        let tgt = target(session, window);
        let mut cmd = Command::new("tmux");
        cmd.args(["capture-pane", "-p", "-t", &tgt]);
        let start;
        if let Some(n) = lines {
            // `-S -N` starts the capture N lines above the visible bottom.
            start = format!("-{n}");
            cmd.args(["-S", &start]);
        }
        let output = cmd.output().context("failed to run 'tmux capture-pane'")?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("tmux capture-pane failed for '{}': {}", tgt, stderr.trim());
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    fn list_windows(&self, session: &str) -> Result<Vec<WindowInfo>> {
        let output = Command::new("tmux")
            .args([
                "list-windows",
                "-t",
                session,
                "-F",
                "#{window_name}\t#{window_active}\t#{pane_dead}",
            ])
            .output()
            .context("failed to run 'tmux list-windows'")?;
        if !output.status.success() {
            // No session / no windows → empty list.
            return Ok(Vec::new());
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut windows = Vec::new();
        for line in stdout.lines() {
            let mut parts = line.split('\t');
            let name = parts.next().unwrap_or_default().to_string();
            if name.is_empty() {
                continue;
            }
            let active = parts.next() == Some("1");
            let dead = parts.next() == Some("1");
            windows.push(WindowInfo { name, active, dead });
        }
        Ok(windows)
    }

    fn kill_window(&self, session: &str, window: &str) -> Result<()> {
        let tgt = target(session, window);
        let output = Command::new("tmux")
            .args(["kill-window", "-t", &tgt])
            .output()
            .context("failed to run 'tmux kill-window'")?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // A window that's already gone is not an error for teardown.
            if stderr.contains("can't find window") || stderr.contains("no such window") {
                return Ok(());
            }
            bail!("tmux kill-window failed for '{}': {}", tgt, stderr.trim());
        }
        Ok(())
    }

    fn attach(&self, session: &str, window: Option<&str>) -> Result<()> {
        if let Some(w) = window {
            // Best-effort: select the task's window before attaching.
            let _ = Command::new("tmux")
                .args(["select-window", "-t", &target(session, w)])
                .status();
        }
        let status = Command::new("tmux")
            .args(["attach-session", "-t", session])
            .status()
            .context("failed to run 'tmux attach-session'")?;
        if !status.success() {
            bail!("tmux attach-session failed for '{session}'");
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Utilities for real backend construction
// ---------------------------------------------------------------------------

/// Check that a binary exists and is executable.
pub fn check_binary(name: &str) -> Result<()> {
    let output = Command::new("which")
        .arg(name)
        .output()
        .with_context(|| format!("failed to run 'which {name}': is 'which' available?"))?;
    if !output.status.success() {
        bail!(
            "required dependency '{name}' not found in PATH; please install it before using nixsand"
        );
    }
    Ok(())
}
