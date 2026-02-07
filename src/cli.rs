use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "ccm", about = "Claude Code Manager - manage multiple Claude Code sessions")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Create a new session (tab + 3-pane layout)
    New {
        /// Session name (optional - will use git branch name if not provided)
        name: Option<String>,
        /// Working directory (defaults to current directory)
        #[arg(long)]
        cwd: Option<String>,
    },
    /// List all sessions
    List,
    /// Switch to a session
    Switch {
        /// Session name
        name: String,
    },
    /// Close a session (alias: exit)
    #[command(alias = "exit")]
    Close {
        /// Session name (optional - detects from current worktree if omitted)
        name: Option<String>,
        /// Merge the branch before closing (calls `gj exit --merge`)
        #[arg(long)]
        merge: bool,
    },
    /// Create a new session from a plan (opens editor)
    Plan {
        /// Working directory (defaults to current directory)
        #[arg(long)]
        cwd: Option<String>,
    },
    /// Initialize config file with defaults
    Init,
    /// Run the tab-watcher TUI sidebar (internal use)
    TabWatcher {
        /// Session name this watcher belongs to
        #[arg(long)]
        session: String,
    },
}
