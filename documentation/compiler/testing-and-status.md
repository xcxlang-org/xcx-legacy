# Tests and current implementation status

## Test organization

| Source | Coverage |
| --- | --- |
| Unit tests in `src/` | Lexer/parser behavior, semantic rules, runtime data structures, collection/lifetime behavior, profiles and CLI parsing |
| [`tests/integration.rs`](../../tests/integration.rs) | Source-level semantic acceptance and positioned diagnostics |
| [`tests/execution.rs`](../../tests/execution.rs) | AOT execution through the library API and expected output |
| [`tests/e2e.rs`](../../tests/e2e.rs) | CLI behavior, native builds, packaged relocation, interactive input, imports, runtime failures and collection lifetimes |
| [`tests/cli.rs`](../../tests/cli.rs) | Effective profiles, optimization parity, custom output, object files, cleanup, invalid flags and source-overwrite protection |
| [`tests/golden.rs`](../../tests/golden.rs) | Full documented examples in debug and release against recorded output/error/exit status |
| [`packing/tests.ps1`](../../packing/tests.ps1) | Archive contents, installer lifecycle, installed native builds, file preservation and malformed-package rejection |

The `.out`, `.err` and `.code` files under `tests/golden` are committed expected
results. They are required test inputs, not leftover output to delete. Golden
comparison normalizes CRLF and supports trailing `*` prefix matches for changing
values such as report dates.

Run compiler tests with matching build and test profiles:

```sh
cargo build --release
cargo test --release
```

The initial build refreshes the top-level static runtime artifact used when linking
test programs. Test compilation alone does not guarantee that artifact is fresh.

## Test coverage

Tests exercise scalar arithmetic, strings and comparisons; conditionals and loops;
functions and recursion; sets, typed maps and table operations; dates and JSON;
input/output, terminal and store operations; source imports and imported JSON text.
There are regression fixtures for block scoping, function return paths, local table
schemas, map reads in expressions, date expressions in filters and managed-object
lifetimes.

## Current limits

| Area | Current state |
| --- | --- |
| Targets | Native host compilation; CI declares three concrete platform/architecture combinations |
| User-function ABI | Only the parameter/return layouts supported by backend value/function modules; not arbitrary collection types |
| Function return inference | Source-order inference without a call-graph fixed point; inferred `date` has no backend return mapping |
| Function scopes | Outer scalar variables/constants are not captured; table filters use a separate capture mechanism |
| Table initial rows | Parser consumes and discards `rows` values; declared tables start empty |
| Array-style composite literals | `>! [1, 2];` is unsupported; arrays are handled in specific operations such as table update |
| Imports | Root resolution uses the working directory; top-level flattening only, without namespaces/cache. Imports in blocks/functions silently do nothing |
| Literal metadata | Map-key and JSON metadata is not kept consistent with all mutations; valid dynamic operations can be rejected |
| JSON numeric binding | An integer JSON number accepted semantically for a float target is rejected by the runtime float accessor |
| Text and arithmetic | String length is UTF-8 byte length; integer arithmetic has no overflow checks |
| Debugging | Frame pointers and compiler verification exist; source-level DWARF/PDB generation is not implemented |
| Diagnostics | First propagated error; line/column/message without a source-file database or multi-error recovery |
| Incremental builds | Each invocation rebuilds its source pipeline; no persistent XCX compilation cache |
| Program arguments | `run` does not forward an arbitrary trailing argument list to the native program |
| Runtime filesystem checks | Lexical relative-path checks; not OS-level isolation or symlink-aware confinement |
| Optimization | Profiles select Cranelift optimizations; performance depends on the program and target |

## Continuous integration

The [CI workflow](../../.github/workflows/ci.yml) builds and tests the compiler in
release mode on Windows x86-64, Linux x86-64 and macOS ARM64. Each job packages the
compiler and runs archive and installer checks on its native platform.

Windows CI checks user PATH addition and removal. Unix jobs check script syntax,
executable permissions and compilation through the installed command symlink.
Package tests also cover reinstallation, self-uninstallation, preservation of extra
files and rejection of incomplete packages or unmanaged installation directories.
