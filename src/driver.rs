use std::process::ExitCode;

use xcx::BuildReport;
use xcx::options::{Cpu, Profile};

use crate::cli::{Action, Invocation};

pub fn execute(invocation: Invocation) -> ExitCode {
    match execute_inner(&invocation) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn execute_inner(invocation: &Invocation) -> Result<ExitCode, String> {
    let source = std::fs::read_to_string(&invocation.source).map_err(|error| {
        format!(
            "error: cannot read `{}`: {error}",
            invocation.source.display()
        )
    })?;
    match invocation.action {
        Action::Run => {
            let report = xcx::run_with_options(&source, &invocation.options)
                .map_err(|error| error.to_string())?;
            if invocation.timings {
                timings(&report.build, invocation);
            }
            Ok(ExitCode::from(report.status.code().unwrap_or(1) as u8))
        }
        Action::Build => {
            let output = invocation.output_path();
            if let (Ok(source), Ok(output)) = (
                std::fs::canonicalize(&invocation.source),
                std::fs::canonicalize(&output),
            ) {
                if source == output {
                    return Err("error: output path would overwrite the source file".into());
                }
            }
            let report = xcx::build_with_options(&source, &output, &invocation.options)
                .map_err(|error| error.to_string())?;
            if !invocation.quiet {
                println!("built `{}`", output.display());
            }
            if invocation.timings {
                timings(&report, invocation);
            }
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn timings(report: &BuildReport, invocation: &Invocation) {
    let profile = match invocation.options.profile {
        Profile::Debug => "debug",
        Profile::Release => "release",
    };
    let cpu = match invocation.options.cpu {
        Cpu::Generic => "generic",
        Cpu::Native => "native",
    };
    eprintln!(
        "timings: profile={profile} opt={} cpu={cpu} frontend={:.3}ms analysis={:.3}ms codegen={:.3}ms link={:.3}ms total={:.3}ms",
        invocation.options.optimization().as_str(),
        report.frontend.as_secs_f64() * 1000.0,
        report.analysis.as_secs_f64() * 1000.0,
        report.codegen.as_secs_f64() * 1000.0,
        report.link.as_secs_f64() * 1000.0,
        report.total().as_secs_f64() * 1000.0,
    );
}
