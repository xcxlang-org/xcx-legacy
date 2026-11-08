# Build, profiles and distribution

## Cargo artifacts and launcher

[`Cargo.toml`](../../Cargo.toml) defines package/library `xcx`, executable
`xcx-legacy`, Rust edition 2024, and both `rlib` and `staticlib` library outputs.
The backend dependencies are Cranelift 0.137.1 components; JSON uses `serde_json`.
The `cc` dependency discovers the native linker toolchain.

[`build.rs`](../../build.rs) writes and compiles a small Rust launcher into
`xcx_launcher.o` using `rustc --emit=obj`, the Cargo target, and `panic=abort`.
The launcher calls `xcx_program_main`, flushes output, finalizes runtime state and
returns an exit code. A flush error takes precedence over the program's code.

The build script embeds the Cargo target and development fallback paths for the
launcher and runtime library in the compiler. The launcher is generated inside
Cargo's build-script output directory. The runtime is `xcx.lib` on Windows and
`libxcx.a` on Unix.

## Code generation and profiles

[`options.rs`](../../src/options.rs) provides `Profile`, `Optimization`, `Cpu`,
`Emit` and `BuildOptions`. The host ISA is built with native CPU-feature inference
enabled only for `Cpu::Native`; `Generic` is the default.

| Setting | Debug | Release |
| --- | --- | --- |
| Default optimization | `none` | `speed` |
| Default register allocator | `single_pass` | `backtracking` |
| Alias analysis | Disabled by default | Enabled by default |
| IR verifier / register-allocation checker | Enabled | Enabled with `--verify` |
| Frame pointers | Preserved | Preserved with `--frame-pointers` |
| Link-time dead-code removal | Not requested | Requested |

Explicit `--opt-level` overrides the optimization, register-allocation and alias
analysis choices. It does not change profile-driven verification, frame-pointer or
linker policy. `size` maps to Cranelift's `speed_and_size`. Unix object generation
enables position-independent code.

`cargo build --release` controls Rust compilation of the compiler and runtime.
`xcx-legacy build/run --release` controls XCX code generation and linking. A compiler
built in Rust release still defaults to the XCX debug profile when invoked without
the command option. The prebuilt runtime retains its own Rust build profile.

## Native linking

[`linker.rs`](../../src/linker.rs) resolves the canonical compiler executable path,
then checks for sidecar files beside it before trying embedded build-time paths.
Canonicalization supports Unix installations reached through a command symlink.

| Platform | Driver and release behavior |
| --- | --- |
| Windows/MSVC | Discover the MSVC toolchain with `cc`, invoke adjacent `link.exe` with the required Windows libraries; release adds `/OPT:REF`, `/OPT:ICF`, `/INCREMENTAL:NO` |
| Linux | Invoke the discovered compiler driver with the object, launcher, runtime and system libraries; release adds `-Wl,--gc-sections` |
| macOS | Invoke the compiler driver with native objects/runtime and required libraries; release adds `-Wl,-dead_strip` |

The output links the XCX runtime into the executable. It does not need an installed
XCX compiler to run, but still uses normal platform system libraries. There is no
CLI cross-compilation target selection.

## Archives and installers

The [CI workflow](../../.github/workflows/ci.yml) declares Windows x86-64, Linux
x86-64 and macOS ARM64 jobs. Each builds and tests Rust release artifacts, creates
an archive with [`packing/package.ps1`](../../packing/package.ps1), tests the
package, and uploads it as an Actions artifact. This workflow does not create a
GitHub Release automatically.

Packaging reads Cargo JSON build events to locate the exact launcher for that build.
It includes the compiler, runtime, launcher, Apache 2.0 license, generated quick-start README and native
install/uninstall scripts. Windows uses ZIP, `.cmd` entry points and `setup.ps1`;
Linux/macOS use `.tar.gz` and Bash scripts. No Python is used.

Unix defaults to a command symlink under `~/.local/bin` and sidecars under
`~/.local/lib/xcx-legacy`. An explicit `/usr/local` prefix supports system-wide
installation. Missing PATH setup is printed for the user to add to their shell.
Windows defaults to `%LOCALAPPDATA%\Programs\xcx-legacy`, with optional user PATH
management. Install markers protect unrelated directories; uninstall removes owned
files and preserves additional user files. See [packaging details](../../packing/README.md).
