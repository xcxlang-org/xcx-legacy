# Compiler implementation

This directory describes the current implementation of `xcx-legacy`, the Rust and
Cranelift AOT compiler reconstructing historical XCX 1.2. It covers the source
modules, compilation pipeline, runtime, supported operations and current limits.

| Document | Scope |
| --- | --- |
| [Architecture](architecture.md) | Entry points, compilation pipeline, module boundaries and library API |
| [Frontend](frontend.md) | Tokens, AST, parsing, imports and diagnostics |
| [Semantic analysis](semantic-analysis.md) | Scopes, types, functions and control-flow checks |
| [Backend and runtime](backend-runtime.md) | Native code generation, value layout, runtime helpers and memory management |
| [Build and distribution](build-and-distribution.md) | Profiles, native linking, launcher, archives and installation |
| [Testing and current status](testing-and-status.md) | Test suites, implemented capabilities and limits |
| [CLI reference](cli.md) | Commands, flags, output paths and exit codes |

The language specification is maintained separately in
[`documentation/language`](../language/README.md). Compiler support and limitations
are described in [Testing and current status](testing-and-status.md).

The Cargo package and Rust library are named `xcx`; the user-facing executable is
`xcx-legacy`. Compiler source, runtime source and the generated launcher are built
as part of the same Cargo project.
