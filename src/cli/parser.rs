use std::collections::HashSet;
use std::ffi::OsString;

use xcx::options::{BuildOptions, Optimization, Profile};

use super::{Action, Command, Invocation};

pub fn parse(arguments: impl IntoIterator<Item = OsString>) -> Result<Command, String> {
    let mut arguments = arguments.into_iter();
    let command = arguments.next().ok_or("a command is required")?;
    let action = match command.to_str() {
        Some("help" | "--help" | "-h") => return standalone(Command::Help, arguments),
        Some("--version" | "-V") => return standalone(Command::Version, arguments),
        Some("build") => Action::Build,
        Some("run") => Action::Run,
        _ => return Err(format!("unknown command `{}`", command.to_string_lossy())),
    };
    let mut source = None;
    let mut output = None;
    let mut options = BuildOptions::default();
    let mut quiet = false;
    let mut timings = false;
    let mut positional = false;
    let mut seen = HashSet::new();
    while let Some(argument) = arguments.next() {
        if !positional && argument == "--" {
            positional = true;
            continue;
        }
        if !positional && argument.as_encoded_bytes().starts_with(b"-") {
            let text = argument
                .to_str()
                .ok_or("option names must be valid Unicode")?;
            let (flag, inline) = text
                .split_once('=')
                .map_or((text, None), |(a, b)| (a, Some(b)));
            if !matches!(flag, "--opt-level" | "--cpu" | "--emit" | "--output") && inline.is_some()
            {
                return Err(format!("option `{flag}` does not accept an inline value"));
            }
            match flag {
                "-h" | "--help" => return Ok(Command::Help),
                "--debug" | "--release" => {
                    once(&mut seen, "profile")?;
                    options.profile = if flag == "--release" {
                        Profile::Release
                    } else {
                        Profile::Debug
                    };
                }
                "-O" | "-O0" | "-Os" => {
                    once(&mut seen, "opt-level")?;
                    options.optimization = Some(match flag {
                        "-O0" => Optimization::None,
                        "-Os" => Optimization::Size,
                        _ => Optimization::Speed,
                    });
                }
                "--opt-level" => {
                    once(&mut seen, "opt-level")?;
                    options.optimization = Some(text_value(flag, inline, &mut arguments)?.parse()?);
                }
                "--cpu" => {
                    once(&mut seen, "cpu")?;
                    options.cpu = text_value(flag, inline, &mut arguments)?.parse()?;
                }
                "--emit" => {
                    once(&mut seen, "emit")?;
                    options.emit = text_value(flag, inline, &mut arguments)?.parse()?;
                }
                "-o" | "--output" => {
                    once(&mut seen, "output")?;
                    output = Some(value(flag, inline, &mut arguments)?.into());
                }
                "--verify" => {
                    once(&mut seen, "verify")?;
                    options.verify = true;
                }
                "--frame-pointers" => {
                    once(&mut seen, "frame-pointers")?;
                    options.frame_pointers = true;
                }
                "--timings" => {
                    once(&mut seen, "timings")?;
                    timings = true;
                }
                "-q" | "--quiet" => {
                    once(&mut seen, "quiet")?;
                    quiet = true;
                }
                _ => return Err(format!("unknown option `{flag}`")),
            }
        } else if source.replace(argument.into()).is_some() {
            return Err("exactly one source file is required".into());
        }
    }
    if action == Action::Run && (output.is_some() || seen.contains("emit")) {
        return Err("--output and --emit are available only for build".into());
    }
    let source: std::path::PathBuf = source.ok_or("a source file is required")?;
    if source.file_stem().is_none() || source.as_os_str().is_empty() {
        return Err("the source path must have a file name".into());
    }
    Ok(Command::Compile(Invocation {
        action,
        source,
        output,
        options,
        timings,
        quiet,
    }))
}

fn standalone(
    command: Command,
    mut rest: impl Iterator<Item = OsString>,
) -> Result<Command, String> {
    if rest.next().is_some() {
        return Err("unexpected arguments after help or version".into());
    }
    Ok(command)
}

fn once(seen: &mut HashSet<&'static str>, name: &'static str) -> Result<(), String> {
    if !seen.insert(name) {
        return Err(format!("conflicting or repeated `{name}` options"));
    }
    Ok(())
}

fn value(
    flag: &str,
    inline: Option<&str>,
    arguments: &mut impl Iterator<Item = OsString>,
) -> Result<OsString, String> {
    let value = match inline {
        Some(value) => OsString::from(value),
        None => arguments
            .next()
            .ok_or_else(|| format!("option `{flag}` requires a value"))?,
    };
    if value.is_empty() || (inline.is_none() && value.as_encoded_bytes().starts_with(b"-")) {
        return Err(format!("option `{flag}` requires a value"));
    }
    Ok(value)
}

fn text_value(
    flag: &str,
    inline: Option<&str>,
    arguments: &mut impl Iterator<Item = OsString>,
) -> Result<String, String> {
    value(flag, inline, arguments)?
        .into_string()
        .map_err(|_| format!("option `{flag}` requires a Unicode value"))
}
