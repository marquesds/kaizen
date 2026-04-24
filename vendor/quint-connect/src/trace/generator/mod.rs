mod run;
mod test;
mod utils;

pub use run::RunConfig;
pub use test::TestConfig;

use crate::trace::iter::Traces;
use anyhow::{Context, Result, anyhow};
use std::{path::Path, process::Command};
use tempfile::TempDir;

/// Default number of traces to generate when not specified.
const DEFAULT_TRACES: usize = 100;

/// Internal trait for configuring trace generation.
pub trait Config {
    fn seed(&self) -> &str;
    fn n_traces(&self) -> usize;
    fn to_command(&self, tmpdir: &Path) -> Command;
}

pub(crate) fn generate_traces<C: Config>(config: &C) -> Result<Traces> {
    let tmpdir = TempDir::with_prefix("quint-connect-")?;
    let mut cmd = config.to_command(tmpdir.path());
    let output = cmd.output().context("Failed to execute Quint command")?;

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Single `anyhow!` (no context chain): the #[quint_run] test uses panic!("{err}"), which
        // would otherwise only print the context wrapper, not the inner I/O.
        return Err(anyhow!(
            "Quint returned non-zero code (status {:?}).\nstdout:\n{stdout}\n\nstderr:\n{stderr}",
            output.status.code()
        ));
    }

    Traces::new(tmpdir)
}
