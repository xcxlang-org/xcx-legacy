use std::env;
use std::path::PathBuf;
use std::process::Command;

const LAUNCHER_SOURCE: &str = r#"#![no_main]

#[unsafe(no_mangle)]
pub extern "C" fn main(_argc: i32, _argv: *const *const u8) -> i32 {
    unsafe extern "C" {
        fn xcx_program_main() -> i64;
        fn xcx_flush_output() -> i32;
        fn xcx_runtime_finish();
    }
    let code = unsafe { xcx_program_main() };
    let flushed = unsafe { xcx_flush_output() };
    unsafe { xcx_runtime_finish(); }
    if flushed != 0 { flushed } else { code as i32 }
}
"#;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is set by cargo"));
    let launcher_source = out_dir.join("xcx_launcher.rs");
    std::fs::write(&launcher_source, LAUNCHER_SOURCE).expect("the launcher source must be writable");

    let launcher_object = out_dir.join("xcx_launcher.o");
    let rustc = env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
    let target = env::var("TARGET").expect("TARGET is set by cargo");
    println!("cargo:rustc-env=XCX_TARGET={target}");
    let status = Command::new(&rustc)
        .arg("--crate-type=bin")
        .arg("--emit=obj")
        .arg("--edition=2024")
        .arg("--target")
        .arg(&target)
        .arg("-Cpanic=abort")
        .arg(&launcher_source)
        .arg("-o")
        .arg(&launcher_object)
        .status()
        .expect("rustc must be runnable from the build script");
    if !status.success() {
        panic!("compiling the program launcher failed");
    }
    println!("cargo:rustc-env=XCX_LAUNCHER_OBJECT={}", launcher_object.display());

    let target_directory = out_dir
        .ancestors()
        .nth(3)
        .expect("OUT_DIR lives under the target directory")
        .to_path_buf();
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let library = target_directory.join("xcx.lib");
        println!("cargo:rustc-env=XCX_RUNTIME_LIBRARY={}", library.display());
    } else {
        let library = target_directory.join("libxcx.a");
        println!("cargo:rustc-env=XCX_RUNTIME_LIBRARY={}", library.display());
    }
}
