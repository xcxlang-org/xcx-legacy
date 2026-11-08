mod cli;
mod driver;

use std::process::ExitCode;

fn main() -> ExitCode {
    match cli::parse(std::env::args_os().skip(1)) {
        Ok(cli::Command::Help) => {
            println!("{}\n\n{}", cli::BANNER, cli::USAGE);
            ExitCode::SUCCESS
        }
        Ok(cli::Command::Version) => {
            println!("xcx-legacy {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Ok(cli::Command::Compile(invocation)) => driver::execute(invocation),
        Err(message) => {
            eprintln!("error: {message}\n\n{}", cli::USAGE);
            ExitCode::from(2)
        }
    }
}
