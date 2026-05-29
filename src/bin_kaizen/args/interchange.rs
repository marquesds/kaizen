use clap::Subcommand;
use std::path::PathBuf;

#[derive(Subcommand)]
pub(crate) enum ExportCommand {
    /// Export one session as ATIF-compatible JSON.
    Atif {
        #[arg(long)]
        session: String,
        #[arg(long)]
        workspace: Option<PathBuf>,
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
    },
}

#[derive(Subcommand)]
pub(crate) enum ImportCommand {
    /// Import ATIF-compatible JSON.
    Atif {
        file: PathBuf,
        #[arg(long)]
        workspace: Option<PathBuf>,
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
    },
    /// Import generic Event JSONL.
    Jsonl {
        file: PathBuf,
        #[arg(long)]
        workspace: Option<PathBuf>,
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
    },
}

#[derive(Subcommand)]
pub(crate) enum VerifyCommand {
    /// Verify per-session event hash chain.
    HashChain {
        #[arg(long)]
        session: Option<String>,
        #[arg(long)]
        workspace: Option<PathBuf>,
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
    },
}
