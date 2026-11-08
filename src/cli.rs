mod parser;

use std::path::PathBuf;

use xcx::options::{BuildOptions, Emit};

pub use parser::parse;

pub const BANNER: &str = "xcx-legacy 1.2 - historical reconstruction AOT compiler";
pub const USAGE: &str = "usage:
  xcx-legacy build [options] <file.xcx>
  xcx-legacy run [options] <file.xcx>

commands:
  build                  AOT-compile a standalone native binary
  run                    AOT-compile a temporary native binary and execute it

options:
  --debug                Debug profile (default): no optimization, full verification
  --release              Release profile: speed optimization and linker dead-code removal
  -O, --opt-level LEVEL   Override optimization: none, speed, size (-O0, -Os also accepted)
  --cpu CPU              Instruction selection: generic (default), native
  --verify               Enable IR and register-allocation verification in release
  --frame-pointers       Preserve frame pointers in release for stack profiling
  -o, --output PATH      Output path (build only)
  --emit KIND            Output kind: exe (default), obj (build only)
  --timings              Report compilation stages and effective settings to stderr
  -q, --quiet            Suppress compiler success messages
  -h, --help             Print help
  -V, --version          Print compiler version
  --                     End options, allowing a source filename starting with '-'
";

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Help,
    Version,
    Compile(Invocation),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Build,
    Run,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Invocation {
    pub action: Action,
    pub source: PathBuf,
    pub output: Option<PathBuf>,
    pub options: BuildOptions,
    pub timings: bool,
    pub quiet: bool,
}

impl Invocation {
    pub fn output_path(&self) -> PathBuf {
        if let Some(output) = &self.output {
            return output.clone();
        }
        let mut name = self.source.file_stem().unwrap_or_default().to_os_string();
        match self.options.emit {
            Emit::Executable => name.push(std::env::consts::EXE_SUFFIX),
            Emit::Object => name.push(if cfg!(windows) { ".obj" } else { ".o" }),
        }
        PathBuf::from(name)
    }
}

#[cfg(test)]
mod tests;
