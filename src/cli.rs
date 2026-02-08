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
    /// Reset the pane layout of the current session tab
    ResetLayout,
    /// Run the tab-watcher TUI sidebar (internal use)
    TabWatcher {
        /// Session name this watcher belongs to
        #[arg(long)]
        session: String,
    },
    /// Watch and display the latest .ccm/plans/*.md file (internal use)
    PlanViewer {
        #[arg(long)]
        cwd: String,
    },
    /// Wrap a command in a PTY, intercepting OSC 0 title changes (internal)
    Wrap {
        /// Session name to update status for
        #[arg(long)]
        session: String,
        /// Optional file whose content is appended as a positional argument to the command
        #[arg(long)]
        prompt_file: Option<String>,
        /// Command and arguments to run
        #[arg(trailing_var_arg = true, required = true)]
        command: Vec<String>,
    },
}
