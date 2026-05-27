use clap::{Args, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "nixsand", about = "Manage isolated coding agent sandboxes")]
pub struct Cli {
    /// Increase verbosity (-v for debug, -vv for trace)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Initialize nixsand home directory and validate dependencies
    Init,

    /// Manage projects
    Project(ProjectArgs),
}

#[derive(Args, Debug)]
pub struct ProjectArgs {
    #[command(subcommand)]
    pub command: ProjectCommands,
}

#[derive(Subcommand, Debug)]
pub enum ProjectCommands {
    /// Add a new project by git URL
    Add {
        /// Git URL to clone
        git_url: String,
        /// Optional project name (defaults to repo basename)
        name: Option<String>,
    },

    /// List all registered projects
    List,

    /// Provision a worktree and container for a branch
    Branch {
        /// Project name
        project: String,
        /// Branch name
        branch: String,
        /// Base branch or commit (defaults to default branch)
        base: Option<String>,
    },

    /// Attach to a branch's tmux session
    Attach {
        /// Project name
        project: String,
        /// Branch name
        branch: String,
    },
}
