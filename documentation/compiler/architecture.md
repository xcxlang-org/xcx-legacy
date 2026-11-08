# Architecture

## Entry points and ownership

| File | Responsibility |
| --- | --- |
| [`src/main.rs`](../../src/main.rs) | Collect OS-native arguments, parse a command, dispatch help/version or compilation, return an exit code |
| [`src/cli.rs`](../../src/cli.rs) | CLI data types, help text and default output filenames |
| [`src/cli/parser.rs`](../../src/cli/parser.rs) | Parse flags and paths; reject unknown, repeated, conflicting and command-incompatible options |
| [`src/driver.rs`](../../src/driver.rs) | Read the source file, protect it from output-path overwrite, invoke the library, print success and timings |
| [`src/lib.rs`](../../src/lib.rs) | Library exports and shared function-return type metadata |
| [`src/compiler.rs`](../../src/compiler.rs) | Coordinate frontend, semantic analysis, object generation and optional linking |
| [`src/options.rs`](../../src/options.rs) | Typed profile, optimization, CPU and emission settings; construct the Cranelift target ISA |
| [`src/execution.rs`](../../src/execution.rs) | Compile a temporary executable and start a child process |
| [`src/workspace.rs`](../../src/workspace.rs) | Own uniquely created temporary directories and remove them on normal scope exit |

CLI argument parsing uses `OsString` and `PathBuf` for file paths. Option names and
enumerated option values require Unicode strings. The driver reads source as UTF-8.

## Compilation flow

`build_with_options` performs these stages in order:

1. Tokenize the source, parse a `Vec<Statement>`, and expand imports.
2. Run semantic analysis and collect inferred function-return information.
3. Create a host ISA from `BuildOptions`; lower the AST directly into Cranelift IR.
4. Emit a native object with `cranelift-object`.
5. For executable emission, invoke the platform linker with the runtime and launcher.

There is no project-owned typed intermediate representation between semantic
analysis and code generation. Semantic analysis returns `Analysis`, whose public
payload is the function-return map; code generation also derives local value and
table-shape information while traversing the AST.

`--emit=obj` writes the native object to the requested output and skips linking.
For executable builds, the intermediate object lives in a temporary workspace.
Neither command implements an incremental compilation cache.

## Execution flow

`run_with_options` requires executable emission. It creates a workspace, calls the
same AOT build pipeline, starts the resulting native executable with inherited
standard input/output, waits for it, and returns its exit status plus `BuildReport`.
The child inherits the caller's working directory; it is not changed to the source
file's directory or the temporary executable's directory.

Workspace names contain a process ID and atomic sequence number. Existing names
are skipped using `create_dir`; `Drop` attempts to remove only the directory created
by that instance. Abrupt termination or filesystem errors can leave temporary files.

## Public library surface

| Function | Actual behavior |
| --- | --- |
| `compile(source)` | Load/import and perform semantic checks; does not generate a native object |
| `build(source, output)` | Build with default debug options; discard the timing report |
| `build_with_options(source, output, options)` | Emit an object or executable and return stage durations |
| `run(source)` | AOT-build with default debug options, capture stdout, and report unsuccessful execution as a diagnostic |
| `run_to_stdout(source)` | Run with default debug options and inherited standard I/O; return `ExitStatus` |
| `run_with_options(source, options)` | Run with explicit options and return `RunReport` |

The captured `run(source)` API uses `Command::output`: stdin is closed, stdout and
stderr are captured, and successful calls return only stdout. Failed calls include
the status and captured stderr in a diagnostic. Interactive programs need the
inherited-I/O execution API instead.

`BuildReport` measures frontend, analysis, codegen and linking. It excludes file
loading by the CLI and child-process execution. `RunReport` contains the build report
and process status. The CLI currently converts normal process exit codes through
`u8`; termination without a normal exit code becomes failure.

For `run --timings`, the CLI prints the completed build report after the child has
exited. The displayed durations still measure compilation, not program execution.
Failed compilations do not return a partial stage report.
