use super::*;
use xcx::options::{Cpu, Optimization, Profile};

fn command(arguments: &[&str]) -> Result<Command, String> {
    parse(arguments.iter().map(std::ffi::OsString::from))
}

fn invocation(arguments: &[&str]) -> Invocation {
    match command(arguments).unwrap() {
        Command::Compile(invocation) => invocation,
        other => panic!("expected a compilation, received {other:?}"),
    }
}

#[test]
fn both_commands_default_to_debug() {
    for action in ["build", "run"] {
        let invocation = invocation(&[action, "test.xcx"]);
        assert_eq!(invocation.options, BuildOptions::default());
        assert_eq!(invocation.source, PathBuf::from("test.xcx"));
    }
}

#[test]
fn flags_can_precede_or_follow_the_source() {
    let before = invocation(&[
        "run",
        "--release",
        "--opt-level=size",
        "--cpu",
        "native",
        "test.xcx",
    ]);
    let after = invocation(&[
        "run",
        "test.xcx",
        "--cpu=native",
        "--opt-level",
        "size",
        "--release",
    ]);
    assert_eq!(before, after);
    assert_eq!(before.options.profile, Profile::Release);
    assert_eq!(before.options.optimization(), Optimization::Size);
    assert_eq!(before.options.cpu, Cpu::Native);
}

#[test]
fn optimization_shorthands_select_real_backend_levels() {
    for (flag, expected) in [
        ("-O", Optimization::Speed),
        ("-O0", Optimization::None),
        ("-Os", Optimization::Size),
    ] {
        assert_eq!(
            invocation(&["build", flag, "test.xcx"])
                .options
                .optimization(),
            expected
        );
    }
}

#[test]
fn object_emission_uses_a_platform_object_extension() {
    let invocation = invocation(&["build", "--emit=obj", "path/test.xcx"]);
    assert_eq!(invocation.options.emit, Emit::Object);
    assert_eq!(
        invocation.output_path(),
        PathBuf::from(if cfg!(windows) { "test.obj" } else { "test.o" })
    );
}

#[test]
fn explicit_output_names_are_preserved() {
    for flag in ["-o", "--output"] {
        assert_eq!(
            invocation(&["build", "test.xcx", flag, "directory/custom name"]).output_path(),
            PathBuf::from("directory/custom name")
        );
    }
    assert_eq!(
        invocation(&["build", "--output=-program", "test.xcx"]).output_path(),
        PathBuf::from("-program")
    );
}

#[test]
fn double_dash_allows_sources_that_start_with_a_dash() {
    let invocation = invocation(&["run", "--release", "--", "-test.xcx"]);
    assert_eq!(invocation.source, PathBuf::from("-test.xcx"));
}

#[test]
fn conflicting_repeated_and_unsupported_options_are_rejected() {
    for arguments in [
        vec!["run", "test.xcx", "--release", "--debug"],
        vec!["run", "test.xcx", "--release", "--release"],
        vec!["run", "test.xcx", "-O", "--opt-level", "size"],
        vec!["build", "test.xcx", "-o", "first", "--output", "second"],
        vec!["run", "test.xcx", "--relase"],
        vec!["run", "test.xcx", "--release=true"],
        vec!["run", "test.xcx", "--cpu", "unknown"],
        vec!["run", "test.xcx", "--opt-level", "best"],
        vec!["build", "test.xcx", "--emit", "asm"],
    ] {
        assert!(command(&arguments).is_err(), "{arguments:?}");
    }
}

#[test]
fn missing_values_and_extra_sources_are_rejected() {
    for arguments in [
        vec![],
        vec!["build"],
        vec!["run", "one.xcx", "two.xcx"],
        vec!["build", "test.xcx", "-o"],
        vec!["build", "test.xcx", "--output="],
        vec!["run", "--opt-level", "--release", "test.xcx"],
        vec!["build", "--cpu=", "test.xcx"],
    ] {
        assert!(command(&arguments).is_err(), "{arguments:?}");
    }
}

#[test]
fn build_only_options_are_rejected_for_run() {
    assert!(command(&["run", "test.xcx", "-o", "out"]).is_err());
    assert!(command(&["run", "test.xcx", "--emit=exe"]).is_err());
    assert!(command(&["run", "test.xcx", "--emit=obj"]).is_err());
}

#[test]
fn quiet_timings_and_profiling_flags_remain_independent() {
    let invocation = invocation(&[
        "run",
        "test.xcx",
        "-q",
        "--timings",
        "--verify",
        "--frame-pointers",
    ]);
    assert!(invocation.quiet);
    assert!(invocation.timings);
    assert!(invocation.options.verify);
    assert!(invocation.options.frame_pointers);
}

#[test]
fn help_and_version_do_not_require_a_source() {
    assert_eq!(command(&["--version"]).unwrap(), Command::Version);
    assert_eq!(command(&["--help"]).unwrap(), Command::Help);
    assert_eq!(command(&["build", "--help"]).unwrap(), Command::Help);
    assert!(command(&["--version", "ignored"]).is_err());
}

#[cfg(unix)]
#[test]
fn non_unicode_source_and_output_paths_are_preserved() {
    use std::os::unix::ffi::OsStringExt;
    let source = std::ffi::OsString::from_vec(b"source-\xff.xcx".to_vec());
    let output = std::ffi::OsString::from_vec(b"output-\xff".to_vec());
    let parsed = parse(["build".into(), source.clone(), "-o".into(), output.clone()]).unwrap();
    let Command::Compile(invocation) = parsed else {
        panic!("expected build")
    };
    assert_eq!(invocation.source.as_os_str(), source);
    assert_eq!(invocation.output_path().as_os_str(), output);
}
