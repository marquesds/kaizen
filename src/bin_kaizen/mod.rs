pub(crate) mod args;
pub(crate) mod dispatch;
pub(crate) mod workspace;

use clap::Parser;

pub fn run() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let cli = args::Cli::parse();
    if cli.no_daemon {
        unsafe { std::env::set_var("KAIZEN_DAEMON", "0") };
    }
    dispatch::run(cli)
}
