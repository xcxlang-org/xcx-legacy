use std::process::{Command, ExitStatus};

use crate::options::{BuildOptions, Emit};
use crate::{BuildReport, Diagnostic, NO_LOCATION, build_with_options, workspace::Workspace};

#[derive(Debug)]
pub struct RunReport {
    pub status: ExitStatus,
    pub build: BuildReport,
}

pub fn run(source: &str) -> Result<String, Diagnostic> {
    let (workspace, _) = prepare(source, &BuildOptions::default())?;
    let output = Command::new(workspace.executable())
        .output()
        .map_err(execution_error)?;
    if !output.status.success() {
        return Err(Diagnostic::new(
            format!(
                "compiled program failed ({}): {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim_end(),
            ),
            NO_LOCATION,
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub fn run_to_stdout(source: &str) -> Result<ExitStatus, Diagnostic> {
    run_with_options(source, &BuildOptions::default()).map(|report| report.status)
}

pub fn run_with_options(source: &str, options: &BuildOptions) -> Result<RunReport, Diagnostic> {
    let (workspace, build) = prepare(source, options)?;
    let status = Command::new(workspace.executable())
        .status()
        .map_err(execution_error)?;
    Ok(RunReport { status, build })
}

fn prepare(source: &str, options: &BuildOptions) -> Result<(Workspace, BuildReport), Diagnostic> {
    if options.emit != Emit::Executable {
        return Err(Diagnostic::new(
            "run requires executable emission",
            NO_LOCATION,
        ));
    }
    let workspace = Workspace::new()?;
    let build = build_with_options(source, &workspace.executable(), options)?;
    Ok((workspace, build))
}

fn execution_error(error: std::io::Error) -> Diagnostic {
    Diagnostic::new(
        format!("cannot run the compiled program: {error}"),
        NO_LOCATION,
    )
}
