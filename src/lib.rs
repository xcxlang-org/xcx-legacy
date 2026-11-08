pub mod backend;
mod compiler;
pub mod diagnostics;
mod execution;
pub mod frontend;
pub mod linker;
pub mod options;
pub mod sema;
mod workspace;

pub use compiler::{BuildReport, build, build_with_options, compile};
pub use diagnostics::Diagnostic;
pub use execution::{RunReport, run, run_to_stdout, run_with_options};

pub type FunctionReturns = std::collections::HashMap<String, sema::types::SemaType>;

const NO_LOCATION: diagnostics::Location = diagnostics::Location { line: 0, column: 0 };
