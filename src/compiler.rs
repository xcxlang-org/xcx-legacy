use std::path::Path;
use std::time::{Duration, Instant};

use crate::frontend::ast::Statement;
use crate::options::{BuildOptions, Emit};
use crate::{Diagnostic, NO_LOCATION, backend, frontend, linker, sema, workspace::Workspace};

#[derive(Clone, Debug, Default)]
pub struct BuildReport {
    pub frontend: Duration,
    pub analysis: Duration,
    pub codegen: Duration,
    pub link: Duration,
}

impl BuildReport {
    pub fn total(&self) -> Duration {
        self.frontend + self.analysis + self.codegen + self.link
    }
}

pub fn compile(source: &str) -> Result<(), Diagnostic> {
    sema::analyze(&load(source)?)?;
    Ok(())
}

pub fn build(source: &str, output_path: &Path) -> Result<(), Diagnostic> {
    build_with_options(source, output_path, &BuildOptions::default()).map(|_| ())
}

pub fn build_with_options(
    source: &str,
    output_path: &Path,
    options: &BuildOptions,
) -> Result<BuildReport, Diagnostic> {
    let start = Instant::now();
    let program = load(source)?;
    let frontend = start.elapsed();
    let start = Instant::now();
    let analysis = sema::analyze(&program)?;
    let analysis_time = start.elapsed();
    let start = Instant::now();
    let isa = options.isa()?;
    let workspace = Workspace::new()?;
    let object_path = if options.emit == Emit::Object {
        output_path.to_path_buf()
    } else {
        workspace.path().join("program.o")
    };
    backend::emit_object(&program, &analysis.function_returns, isa, &object_path)?;
    let codegen = start.elapsed();
    let start = Instant::now();
    if options.emit == Emit::Executable {
        linker::link_with_options(&object_path, output_path, options)
            .map_err(|message| Diagnostic::new(message, NO_LOCATION))?;
    }
    let link = if options.emit == Emit::Executable {
        start.elapsed()
    } else {
        Duration::ZERO
    };
    Ok(BuildReport {
        frontend,
        analysis: analysis_time,
        codegen,
        link,
    })
}

fn load(source: &str) -> Result<Vec<Statement>, Diagnostic> {
    let tokens = frontend::lexer::tokenize(source)?;
    let program = frontend::parser::parse(tokens)?;
    frontend::imports::expand(program)
}
