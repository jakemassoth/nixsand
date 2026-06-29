use anyhow::{bail, Context, Result};

use crate::config::Config;
use crate::names::{name_from_url, validate_project_name};

// ---------------------------------------------------------------------------
// project add
// ---------------------------------------------------------------------------

pub fn run_add(config: &Config, git_url: &str, name: Option<&str>) -> Result<()> {
    // Derive project name from URL if not provided
    let project_name = match name {
        Some(n) => n.to_string(),
        None => name_from_url(git_url),
    };

    validate_project_name(&project_name)?;

    // Check for duplicate
    if config.store.project_exists(&project_name)? {
        bail!(
            "project '{project_name}' already exists; choose a different name or remove the existing project"
        );
    }

    // Create project directory structure
    let bare_dir = config.bare_repo_dir(&project_name);
    let worktrees_dir = config.worktrees_dir(&project_name);

    std::fs::create_dir_all(&worktrees_dir).with_context(|| {
        format!(
            "failed to create project directory at {}",
            worktrees_dir.display()
        )
    })?;

    // Bare clone
    eprintln!("[add] cloning {} into {}...", git_url, bare_dir.display());
    config
        .git
        .clone_bare(git_url, &bare_dir)
        .with_context(|| format!("failed to clone '{git_url}'"))?;

    // Set relative worktree paths so a worktree's `.git` pointer stays valid
    // even if the project tree is moved. `git worktree add` is what actually
    // auto-writes `extensions.relativeWorktrees = true` (which some libgit2
    // consumers reject); `run_spawn` strips that after each add.
    config
        .git
        .set_config(&bare_dir, "worktree.useRelativePaths", "true")
        .context("failed to configure worktree.useRelativePaths")?;

    // Register in DB
    config
        .store
        .add_project(&project_name, git_url)
        .with_context(|| format!("failed to register project '{project_name}'"))?;

    println!("project '{project_name}' added ({git_url})");
    println!("run 'nixsand spawn {project_name} <branch>' to start an agent");
    Ok(())
}

// ---------------------------------------------------------------------------
// project list
// ---------------------------------------------------------------------------

pub fn run_list(config: &Config) -> Result<()> {
    let projects = config.store.list_projects()?;
    if projects.is_empty() {
        println!("no projects registered; run 'nixsand project add <git-url>' to add one");
    } else {
        for (name, url) in &projects {
            println!("{name}\t{url}");
        }
    }
    Ok(())
}
