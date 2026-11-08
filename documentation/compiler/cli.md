# xcx-legacy CLI

`xcx-legacy` compiles historical XCX 1.2 to native machine code ahead of execution.
Windows/MSVC, Linux/GCC and macOS/Clang use their native object and executable formats.
Each compiler targets its own operating system and architecture.

```sh
xcx-legacy run program.xcx
xcx-legacy run --release program.xcx
xcx-legacy build program.xcx
xcx-legacy build --release program.xcx -o application
xcx-legacy build --release --opt-level=size program.xcx
xcx-legacy run program.xcx --release --cpu=native --timings
xcx-legacy build --emit=obj program.xcx -o program.o
```

Options may appear before or after the source path. Exactly one source is required.
Use `--` before a source filename beginning with a hyphen. On Windows the default
executable filename ends in `.exe`; custom `-o` filenames are used exactly as supplied.
Default object output is `<stem>.obj` on Windows and `<stem>.o` on Linux and macOS.
Outputs without `-o` are written to the current working directory.

| Option | Behavior |
| --- | --- |
| `--debug` | Default profile: no optimization, fast register allocation, IR/register-allocation verification and frame pointers. |
| `--release` | Speed optimization, alias analysis, backtracking register allocation, linker dead-code removal and Windows identical COMDAT folding. |
| `-O`, `--opt-level=speed` | Optimize generated code for speed. |
| `-O0`, `--opt-level=none` | Disable most generated-code optimizations. |
| `-Os`, `--opt-level=size` | Optimize for speed and size. |
| `--cpu=generic` | Default: architecture baseline instructions for generated XCX code. |
| `--cpu=native` | Enable host CPU features; the resulting program may require the same CPU features elsewhere. |
| `--verify` | Enable IR and register-allocation verification in release. |
| `--frame-pointers` | Preserve generated-code frame pointers in release for stack profiling. |
| `-o PATH`, `--output=PATH` | Choose the build output path. |
| `--emit=exe` | Build a linked standalone executable (default). |
| `--emit=obj` | Emit a native relocatable object without invoking the linker. Build only. |
| `--timings` | Print effective profile, optimization and CPU selection, plus frontend, analysis, codegen and linker times, to stderr. Runtime execution time is excluded. |
| `-q`, `--quiet` | Suppress compiler success messages. Program output, diagnostics and explicitly requested timings remain visible. |
| `-h`, `--help` | Display command help. |
| `-V`, `--version` | Display the compiler version. |

An explicit optimization level overrides the profile's default without changing the
profile's verification, frame-pointer or linker policies. Duplicate settings and
conflicting profiles are errors. `--output` and `--emit` are rejected for `run`.
The previous `XCX_OPT` environment variable is replaced by explicit CLI options.

The debug profile does not currently emit source-level DWARF or PDB information.
`cargo build --release` optimizes the Rust compiler and packaged runtime; the
`xcx-legacy --release` command option independently controls generated XCX code.
The linked runtime retains the Rust profile with which it was built.

`run` always compiles and links a temporary executable, then starts it as a separate
process with inherited standard input/output. Temporary files are removed after
compilation errors, normal completion and runtime errors. Both commands require the
platform linker except `build --emit=obj`. Native builds need the packaged runtime
static library and launcher object next to the compiler.

CLI usage errors exit with code 2. Compilation and launch failures exit with code 1.
`run` forwards the program's exit code for normal exit codes in the 0-255 range;
termination without an exit code is reported as failure. Compilation timings and
diagnostics go to stderr. Successful `build` prints its output filename unless quiet.
