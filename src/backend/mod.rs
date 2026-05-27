use std::path::Path;
use anyhow::Result;

pub mod real;
pub mod mock;

/// A bind mount specification.
#[derive(Debug, Clone)]
pub struct Mount {
    pub host_path: String,
    pub container_path: String,
}

/// Trait abstracting container operations.
#[allow(dead_code)]
pub trait ContainerBackend: Send + Sync {
    fn image_exists(&self, tag: &str) -> Result<bool>;
    fn build_image(&self, tag: &str, context_dir: &Path) -> Result<()>;
    fn container_exists(&self, name: &str) -> Result<bool>;
    fn container_running(&self, name: &str) -> Result<bool>;
    fn create_container(
        &self,
        name: &str,
        image: &str,
        mounts: &[Mount],
        entrypoint: &[&str],
    ) -> Result<()>;
    fn start_container(&self, name: &str) -> Result<()>;
    fn remove_container(&self, name: &str) -> Result<()>;
    /// Run an interactive command inside a container (for attach).
    fn exec_interactive(&self, name: &str, command: &str) -> Result<()>;
}

/// Trait abstracting git operations.
pub trait GitBackend: Send + Sync {
    fn clone_bare(&self, url: &str, dest: &Path) -> Result<()>;
    fn set_config(&self, repo: &Path, key: &str, value: &str) -> Result<()>;
    fn add_worktree(
        &self,
        bare_repo: &Path,
        worktree_path: &Path,
        branch: &str,
        base: &str,
    ) -> Result<()>;
    fn default_branch(&self, bare_repo: &Path) -> Result<String>;
    /// Read a file from the repository's working tree (via git show).
    fn read_file(&self, repo: &Path, path: &str) -> Result<Vec<u8>>;
}

/// Trait abstracting tmux terminal multiplexer operations.
pub trait ZmxBackend: Send + Sync {
    fn session_exists(&self, session: &str) -> Result<bool>;
    fn new_session(&self, session: &str, command: &str) -> Result<()>;
    fn attach_session(&self, session: &str) -> Result<()>;
}
