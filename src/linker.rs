use crate::options::{BuildOptions, Profile};
use std::path::{Path, PathBuf};
use std::process::Command;

const BAKED_LAUNCHER_OBJECT: Option<&str> = option_env!("XCX_LAUNCHER_OBJECT");
const BAKED_RUNTIME_LIBRARY: Option<&str> = option_env!("XCX_RUNTIME_LIBRARY");

pub fn link(object_path: &Path, output_path: &Path) -> Result<(), String> {
    link_with_options(object_path, output_path, &BuildOptions::default())
}

pub fn link_with_options(
    object_path: &Path,
    output_path: &Path,
    options: &BuildOptions,
) -> Result<(), String> {
    let launcher = locate(BAKED_LAUNCHER_OBJECT, "xcx_launcher.o").ok_or(
        "the program launcher object was not found — the compiler installation is incomplete",
    )?;
    let runtime_library = locate(BAKED_RUNTIME_LIBRARY, runtime_library_name())
        .ok_or("the runtime library was not found — the compiler installation is incomplete")?;

    let mut command = compiler_driver()?;
    match std::env::consts::OS {
        "windows" => {
            let driver = PathBuf::from(command.get_program());
            let link_exe = driver
                .parent()
                .ok_or("the MSVC compiler driver has no directory")?
                .join("link.exe");
            let mut linked = Command::new(link_exe);
            for (key, value) in command.get_envs() {
                match value {
                    Some(value) => {
                        linked.env(key, value);
                    }
                    None => {
                        linked.env_remove(key);
                    }
                }
            }
            linked.arg("/NOLOGO");
            linked.args(optimization_arguments("windows", options.profile));
            for library in ["msvcrt.lib", "vcruntime.lib", "ucrt.lib", "kernel32.lib"] {
                linked.arg(format!("/DEFAULTLIB:{}", library));
            }
            linked.arg(object_path);
            linked.arg(&launcher);
            linked.arg(&runtime_library);
            for library in [
                "kernel32.lib",
                "advapi32.lib",
                "ntdll.lib",
                "userenv.lib",
                "ws2_32.lib",
                "dbghelp.lib",
                "ole32.lib",
                "oleaut32.lib",
                "legacy_stdio_definitions.lib",
                "vcruntime.lib",
            ] {
                linked.arg(library);
            }
            linked.arg(format!("/OUT:{}", output_path.display()));
            command = linked;
        }
        _ => {
            command.arg(object_path);
            command.arg(&launcher);
            command.arg(&runtime_library);
            command.arg("-lm").arg("-lpthread");
            command.args(optimization_arguments(
                std::env::consts::OS,
                options.profile,
            ));
            if cfg!(target_os = "linux") {
                command.args(["-ldl", "-lrt", "-lutil"]);
            } else if cfg!(target_os = "macos") {
                command.arg("-liconv");
            }
            command.arg("-o").arg(output_path);
        }
    }

    let run = command.output().map_err(|error| {
        format!(
            "cannot run the linker driver `{}`: {error}",
            command.get_program().to_string_lossy()
        )
    })?;
    if !run.status.success() {
        let stdout = String::from_utf8_lossy(&run.stdout);
        let stderr = String::from_utf8_lossy(&run.stderr);
        return Err(format!("linking failed:\n{}{}", stdout, stderr));
    }
    Ok(())
}

fn optimization_arguments(platform: &str, profile: Profile) -> &'static [&'static str] {
    match (platform, profile) {
        ("windows", Profile::Release) => &["/OPT:REF", "/OPT:ICF", "/INCREMENTAL:NO"],
        ("windows", Profile::Debug) => &["/OPT:NOREF", "/OPT:NOICF", "/INCREMENTAL:NO"],
        ("linux", Profile::Release) => &["-Wl,--gc-sections"],
        ("macos", Profile::Release) => &["-Wl,-dead_strip"],
        _ => &[],
    }
}

fn locate(baked: Option<&str>, beside_compiler: &str) -> Option<PathBuf> {
    let current = std::env::current_exe().ok()?.canonicalize().ok()?;
    let candidate = current.parent()?.join(beside_compiler);
    if candidate.is_file() {
        return Some(candidate);
    }
    if let Some(baked) = baked {
        let baked = PathBuf::from(baked);
        if baked.is_file() {
            return Some(baked);
        }
    }
    None
}

fn runtime_library_name() -> &'static str {
    match std::env::consts::DLL_EXTENSION {
        "dll" => "xcx.lib",
        _ => "libxcx.a",
    }
}

fn compiler_driver() -> Result<Command, String> {
    let target = env!("XCX_TARGET");
    cc::Build::new()
        .opt_level(0)
        .host(target)
        .target(target)
        .cargo_metadata(false)
        .try_get_compiler()
        .map(|compiler| compiler.to_command())
        .map_err(|error| format!("cannot locate the platform linker toolchain: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_linking_removes_dead_code_on_each_supported_platform() {
        assert!(optimization_arguments("windows", Profile::Release).contains(&"/OPT:REF"));
        assert!(optimization_arguments("windows", Profile::Release).contains(&"/OPT:ICF"));
        assert_eq!(
            optimization_arguments("linux", Profile::Release),
            &["-Wl,--gc-sections"]
        );
        assert_eq!(
            optimization_arguments("macos", Profile::Release),
            &["-Wl,-dead_strip"]
        );
    }

    #[test]
    fn debug_linking_preserves_code_for_inspection() {
        assert!(optimization_arguments("windows", Profile::Debug).contains(&"/OPT:NOREF"));
        assert!(optimization_arguments("linux", Profile::Debug).is_empty());
        assert!(optimization_arguments("macos", Profile::Debug).is_empty());
    }
}
