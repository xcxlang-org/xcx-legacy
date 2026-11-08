# xcx-legacy

A native AOT compiler for historical XCX 1.2, written in Rust using Cranelift.

The implementation is experimental and does not yet support every form of XCX 1.2.
See [current implementation limits](documentation/compiler/testing-and-status.md#current-limits).

This project reconstructs XCX 1.2 (official beta, February 7, 2026) from its preserved
language specification. Development uses AI assistance, with no implementation code
carried over from the original Python interpreter and C transpiler.

The current XCX language is available at [xcxlang.com](https://xcxlang.com/) and
[xcxlang-org/xcx](https://github.com/xcxlang-org/xcx).

## Supported platforms

| Platform | Release target | Linker toolchain |
| --- | --- | --- |
| Windows | `x86_64-pc-windows-msvc` | MSVC Build Tools and Windows SDK |
| Linux | `x86_64-unknown-linux-gnu` | GCC |
| macOS | `aarch64-apple-darwin` | Clang / Xcode Command Line Tools |

Each compiler produces native programs for its own operating system and architecture.
Windows executables use `.exe`; Linux and macOS executables have no required suffix.

## Build and try

Install stable Rust and the platform linker toolchain, then build the compiler and
its runtime:

```sh
cargo build --release
```

Run an existing example on Windows:

```powershell
.\target\release\xcx-legacy.exe run .\tests\fixtures\run\functions.xcx --release
```

On Linux or macOS:

```sh
./target/release/xcx-legacy run tests/fixtures/run/functions.xcx --release
```

The example exercises functions, recursion and strings, including `5! = 120`.

## Commands

When the compiler is on your `PATH`:

```sh
xcx-legacy run tests/fixtures/run/functions.xcx
xcx-legacy run --release tests/fixtures/run/functions.xcx
xcx-legacy build --release tests/fixtures/run/functions.xcx
xcx-legacy build --release --opt-level=size tests/fixtures/run/functions.xcx
xcx-legacy build --emit=obj tests/fixtures/run/functions.xcx
xcx-legacy --help
xcx-legacy --version
```

`build` creates a standalone native executable, or a relocatable object with
`--emit=obj`. Default output names use the source filename without its `.xcx`
extension. Object files use `.obj` on Windows and `.o` on Linux and macOS. Outputs
are written to the current working directory; `-o PATH` sets an exact output path.

`run` compiles and links a temporary native executable, starts it as a separate
process with inherited standard input/output, and removes temporary files afterward.

## Profiles and optimization

| Profile | Selection | Generated code |
| --- | --- | --- |
| Debug | Default, or `--debug` | Optimization disabled, fast register allocation, IR/register-allocation verification and preserved frame pointers |
| Release | `--release` | Speed optimization, alias analysis, backtracking register allocation and linker dead-code removal |

`cargo build --release` optimizes the Rust compiler and runtime. The command option
`xcx-legacy run --release` or `xcx-legacy build --release` independently selects the
profile used to generate XCX machine code.

| Option | Purpose |
| --- | --- |
| `--opt-level none\|speed\|size` | Override the profile's default optimization level |
| `-O0`, `-O`, `-Os` | Optimization shortcuts for none, speed, and speed plus size |
| `--cpu generic\|native` | Architecture baseline instructions (default), or host CPU features |
| `-o PATH`, `--output PATH` | Choose the build output path |
| `--emit exe\|obj` | Emit a linked executable (default) or a native object; build only |
| `--verify` | Enable IR and register-allocation verification in release |
| `--frame-pointers` | Preserve frame pointers in release for stack profiling |
| `--timings` | Report effective settings and compilation stage times to stderr |
| `-q`, `--quiet` | Suppress compiler success messages |

Options may appear before or after the source path. Timings exclude program
execution. An explicit optimization level keeps the selected profile's verification,
frame-pointer and linker policies.

See the [compiler implementation documentation](documentation/compiler/README.md)
for the architecture, module map, runtime and current limitations, and the
[CLI reference](documentation/compiler/cli.md) for command behavior and exit codes.

## Distribution

A compiler installation contains three files in the same directory:

| Artifact | Windows | Linux / macOS |
| --- | --- | --- |
| Compiler | `xcx-legacy.exe` | `xcx-legacy` |
| Runtime static library | `xcx.lib` | `libxcx.a` |
| Launcher object | `xcx_launcher.o` | `xcx_launcher.o` |

The runtime is linked into generated executables. Running a built program does not
require the XCX compiler. Compiling and linking programs requires the platform linker
toolchain; object-only emission skips the linker.

Release archives include a quick-start README and install/uninstall scripts. See
[packaging](packing/README.md) for archive formats and installation paths.
GitHub Actions builds, tests and packages the three supported release targets.

## Tests

```sh
cargo build --release
cargo test --release
```

The build step refreshes the runtime static library. Tests alone do not refresh
Cargo's top-level runtime library artifact.

Tests cover parsing, semantic analysis, code generation, native execution, CLI
validation, profiles, object emission, interactive input, temporary-file cleanup and
packaged compiler behavior. The documented example corpus is checked against
recorded stdout, stderr and exit codes in both debug and release profiles.

## Current limits

- Cross-compilation is not implemented.
- Debug builds do not currently emit source-level DWARF or PDB information.
- Native CPU selection can require matching CPU features on another machine.
- The linked runtime retains the Rust build profile used to create it.

## Project layout

| Path | Contents |
| --- | --- |
| `src/cli/`, `src/cli.rs`, `src/driver.rs` | Command parsing and CLI execution |
| `src/options/`, `src/options.rs` | Profiles, optimization and target CPU settings |
| `src/compiler.rs`, `src/execution.rs` | AOT pipeline and native process execution |
| `src/frontend/`, `src/sema/` | Parsing, imports and semantic analysis |
| `src/backend/`, `src/linker.rs` | Cranelift code generation, runtime and native linking |
| `tests/` | Unit, integration, execution, CLI and documented-example tests |
| `documentation/language/` | Preserved XCX 1.2 language documentation |
| `documentation/compiler/` | Compiler architecture, modules, runtime, CLI and implementation status |
| `packing/` | Distribution templates, installers and package validation |

The historical specification is preserved in
[the language documentation](documentation/language/README.md).

## License

Licensed under the [Apache License 2.0](LICENSE).
