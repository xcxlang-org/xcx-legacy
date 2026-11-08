mod common;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use common::fixture_path;

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

struct Workspace(PathBuf);

impl Workspace {
    fn new() -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("xcx-legacy-cli-{}-{id}", std::process::id()));
        std::fs::create_dir(&path).expect("create workspace");
        Self(path)
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn compiler() -> Command {
    Command::new(env!("CARGO_BIN_EXE_xcx-legacy"))
}

fn success(output: &Output) {
    assert!(
        output.status.success(),
        "status: {}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn run_fixture(fixture: &str, flags: &[&str]) -> Output {
    compiler()
        .arg("run")
        .arg(fixture_path(fixture))
        .args(flags)
        .output()
        .unwrap()
}

#[test]
fn version_and_help_use_the_legacy_binary_name() {
    let output = compiler().arg("--version").output().unwrap();
    success(&output);
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "xcx-legacy 1.2.0\n"
    );
    let output = compiler().args(["run", "--help"]).output().unwrap();
    success(&output);
    let help = String::from_utf8(output.stdout).unwrap();
    assert!(help.contains("xcx-legacy build"));
    assert!(help.contains("--release"));
}

#[test]
fn run_defaults_to_debug_and_release_selects_speed() {
    for (flags, expected) in [
        (vec!["--timings"], "profile=debug opt=none"),
        (vec!["--release", "--timings"], "profile=release opt=speed"),
        (
            vec!["--release", "--opt-level=none", "--timings"],
            "profile=release opt=none",
        ),
        (
            vec!["--debug", "-Os", "--timings"],
            "profile=debug opt=size",
        ),
    ] {
        let output = run_fixture("run/arithmetic.xcx", &flags);
        success(&output);
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            "13\n1\n-3\n48123456789\n31\n"
        );
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains(expected), "{stderr}");
        for stage in ["frontend=", "analysis=", "codegen=", "link=", "total="] {
            assert!(stderr.contains(stage), "{stderr}");
        }
    }
}

#[test]
fn optimized_profiles_preserve_runtime_and_control_flow_semantics() {
    for fixture in [
        "run/floats.xcx",
        "run/functions.xcx",
        "run/loops.xcx",
        "run/maps.xcx",
        "run/tables.xcx",
        "run/json.xcx",
        "run/dates.xcx",
        "run/halt-function.xcx",
        "run-invalid/missing-map-key-runtime.xcx",
        "valid/memory-lifetimes.xcx",
    ] {
        let debug = run_fixture(fixture, &[]);
        for flags in [
            vec!["--release"],
            vec!["--release", "-Os", "--verify", "--frame-pointers"],
        ] {
            let optimized = run_fixture(fixture, &flags);
            assert_eq!(
                debug.status.code(),
                optimized.status.code(),
                "{fixture}: {flags:?}"
            );
            assert_eq!(debug.stdout, optimized.stdout, "{fixture}: {flags:?}");
            assert_eq!(debug.stderr, optimized.stderr, "{fixture}: {flags:?}");
        }
    }
}

#[test]
fn custom_output_quiet_mode_and_release_build_produce_a_runnable_binary() {
    let workspace = Workspace::new();
    let binary = workspace
        .0
        .join(format!("custom name{}", std::env::consts::EXE_SUFFIX));
    let output = compiler()
        .arg("build")
        .arg(fixture_path("run/functions.xcx"))
        .args(["--release", "--cpu=native", "--quiet", "--timings", "-o"])
        .arg(&binary)
        .output()
        .unwrap();
    success(&output);
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("profile=release opt=speed cpu=native")
    );
    let native = Command::new(&binary).output().unwrap();
    success(&native);
    assert_eq!(native.stdout, run_fixture("run/functions.xcx", &[]).stdout);
}

#[test]
fn build_defaults_to_debug_and_object_emission_does_not_invoke_the_linker() {
    let workspace = Workspace::new();
    for (flags, profile) in [(vec![], "debug"), (vec!["--release", "-Os"], "release")] {
        let object = workspace.0.join(format!("{profile}.o"));
        let output = compiler()
            .arg("build")
            .arg(fixture_path("run/functions.xcx"))
            .args(flags)
            .args(["--emit=obj", "--timings", "-o"])
            .arg(&object)
            .env("CC", "xcx-linker-must-not-be-invoked")
            .output()
            .unwrap();
        success(&output);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(&format!("profile={profile}")), "{stderr}");
        assert!(stderr.contains("link=0.000ms"), "{stderr}");
        assert_native_object(&std::fs::read(&object).unwrap());
    }
}

fn assert_native_object(bytes: &[u8]) {
    assert!(bytes.len() > 64);
    match std::env::consts::OS {
        "windows" => assert_eq!(
            &bytes[..2],
            if cfg!(target_arch = "aarch64") {
                &[0x64, 0xaa]
            } else {
                &[0x64, 0x86]
            }
        ),
        "linux" => assert_eq!(&bytes[..4], b"\x7fELF"),
        "macos" => assert_eq!(&bytes[..4], &[0xcf, 0xfa, 0xed, 0xfe]),
        platform => panic!("unsupported test platform: {platform}"),
    }
}

#[test]
fn invalid_cli_arguments_fail_without_creating_outputs() {
    let workspace = Workspace::new();
    for flags in [
        vec!["--release", "--debug"],
        vec!["--emit=asm"],
        vec!["--cpu=unknown"],
        vec!["--relase"],
    ] {
        let output = compiler()
            .arg("build")
            .arg(fixture_path("run/arithmetic.xcx"))
            .args(flags)
            .current_dir(&workspace.0)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
    assert_eq!(std::fs::read_dir(&workspace.0).unwrap().count(), 0);
}

#[test]
fn output_cannot_replace_the_source() {
    let workspace = Workspace::new();
    let source = workspace.0.join("source.xcx");
    std::fs::copy(fixture_path("run/arithmetic.xcx"), &source).unwrap();
    let original = std::fs::read(&source).unwrap();
    let output = compiler()
        .arg("build")
        .arg(&source)
        .arg("-o")
        .arg(&source)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("overwrite the source"));
    assert_eq!(std::fs::read(source).unwrap(), original);
}

#[test]
fn run_cleans_temporary_files_after_success_compile_error_and_runtime_error() {
    let workspace = Workspace::new();
    for fixture in [
        "run/functions.xcx",
        "invalid/type-mismatch.xcx",
        "run-invalid/missing-map-key-runtime.xcx",
    ] {
        let output = compiler()
            .arg("run")
            .arg(fixture_path(fixture))
            .env("TMPDIR", &workspace.0)
            .env("TMP", &workspace.0)
            .env("TEMP", &workspace.0)
            .output()
            .unwrap();
        if fixture.starts_with("run/") {
            success(&output);
        } else {
            assert_eq!(output.status.code(), Some(1));
        }
        assert_eq!(
            std::fs::read_dir(&workspace.0).unwrap().count(),
            0,
            "{fixture}"
        );
    }
}

#[test]
fn run_accepts_a_dash_prefixed_source_after_the_option_terminator() {
    let workspace = Workspace::new();
    std::fs::copy(
        fixture_path("run/arithmetic.xcx"),
        workspace.0.join("-source.xcx"),
    )
    .unwrap();
    let output = compiler()
        .args(["run", "--release", "--", "-source.xcx"])
        .current_dir(Path::new(&workspace.0))
        .output()
        .unwrap();
    success(&output);
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "13\n1\n-3\n48123456789\n31\n"
    );
}
