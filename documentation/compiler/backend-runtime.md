# Backend, ABI and runtime

## Code-generation modules

| File | Responsibility |
| --- | --- |
| [`backend.rs`](../../src/backend.rs) | Create `ObjectModule`, declare/define user functions and `xcx_program_main`, collect shared metadata, emit object bytes |
| [`backend/codegen.rs`](../../src/backend/codegen.rs) | Per-function generation state, locals, values, scopes, statement/expression lowering and runtime roots |
| [`backend/codegen/operators.rs`](../../src/backend/codegen/operators.rs) | Generate arithmetic, comparisons, logical and string operations |
| [`backend/control_flow.rs`](../../src/backend/control_flow.rs) | Generate branch and loop blocks, breaks and continues |
| [`backend/functions.rs`](../../src/backend/functions.rs) | Function registry, machine signatures, parameter/return layouts and table-return shapes |
| [`backend/calls.rs`](../../src/backend/calls.rs) | Lower calls and returns using those signatures |
| [`backend/composites.rs`](../../src/backend/composites.rs) | Generate set/map literals and pack collection elements |
| [`backend/methods.rs`](../../src/backend/methods.rs) | Dispatch methods and built-in namespace calls during lowering |
| [`backend/methods/tables.rs`](../../src/backend/methods/tables.rs) | Generate table declarations, construction and operations |
| [`backend/methods/filter.rs`](../../src/backend/methods/filter.rs) | Generate table-filter predicates and callbacks |
| [`backend/values.rs`](../../src/backend/values.rs) | Storage classification and ABI slot/return shapes |

`FunctionCodegen` owns a Cranelift `FunctionBuilder` and references the current module,
runtime helper declarations, function registry, table schemas and constant data.
Its `Local` and `Val` enums distinguish scalar, string, date, collection, table-row
and JSON cases. `Flow` tracks whether lowering has terminated the current path.

Functions and data objects are emitted into individual sections. Identical constant
byte sequences share data IDs within the object. The target's calling convention is
used for generated function signatures. Cranelift performs machine-code optimization
and register allocation according to `BuildOptions`.

## Machine values and function ABI

| Value | Current representation |
| --- | --- |
| Integer | `I64` |
| Float | `F64` |
| Boolean | `I8` |
| Internal character values | `I32` |
| String | Pointer and byte length, passed in two `I64` slots |
| Date | Day count in `I64` |
| Sets, maps, tables and JSON objects | Opaque runtime pointers/handles represented by `I64` |

String-returning functions receive an additional output pointer and write their
result through that ABI rather than returning a machine scalar. Table returns use
an opaque pointer with schema metadata tracked by the compiler. Function symbols
are named `xcx_user_<name>`; generated top-level code is `xcx_program_main`.

Parameter layouts currently support integers, floats, booleans, strings, tables and
dates. Set, map and JSON storage support does not imply support for those types in
user-function parameters or returns. This internal ABI is not a public foreign
function interface or a compatibility guarantee across compiler versions.

Explicit return layouts support those same scalar/string/table/date types. The
separate inferred-return mapping supports integer, float, boolean, string and table,
but omits date. Table parameters carry an opaque handle without a known column
shape; declaring such a parameter does not provide schema-dependent operations
such as named column access or insertion. Inferred table returns can carry schema
metadata, which is a different code path.

## Operation and data-structure behavior

| Area | Implementation and consequence |
| --- | --- |
| Strings | UTF-8 pointer/byte-length pairs; `.length()` returns bytes, and comparisons operate on bytes |
| Integer arithmetic | Plain `I64` addition, subtraction and multiplication; no checked-overflow diagnostic. Maximum integer plus one wraps to the minimum integer |
| Logical operators | `and`/`or` use conditional control flow for short-circuit evaluation |
| Sets | Vector-backed elements and eagerly materialized ranges; membership scans elements and algebra repeatedly uses membership checks. Literal construction does not remove duplicates |
| Maps | Vector-backed entries with linear lookup and mutation; generated map keys are integer or string |
| Tables | Vectors of rows/cells; joins use nested row scans. Deleting a row shifts later row indexes |
| Table IDs | Automatic `id` starts at 1, while row indexes start at 0; update rejects changes to the `id` column |
| JSON | `serde_json::Value`; object navigation uses dot-separated keys and clones accessed subtrees |
| Dates | Gregorian day counts; `date.now()` uses the UTC day from Unix-epoch time, with no sub-day clock API |

These are concrete implementation choices, not constant-time collection or general
query-optimizer guarantees. Table filtering lowers a predicate into a callback with
an explicit capture context; that special mechanism is not a user-function closure
ABI. Standalone arrays remain unsupported despite their use as table-update payloads.

## Runtime modules

| File | Responsibility |
| --- | --- |
| [`backend/runtime.rs`](../../src/backend/runtime.rs) | Runtime symbol signatures, Cranelift helper declarations, scalar/string helpers, output, flushing, HALT and finalization |
| [`backend/collections.rs`](../../src/backend/collections.rs) | Set objects, builders, ranges, algebra, membership and selection |
| [`backend/maps.rs`](../../src/backend/maps.rs) | Map objects, typed entries, key iteration and mutation |
| [`backend/tables.rs`](../../src/backend/tables.rs) | Table schemas/cells, rows, CRUD, filtering/join helpers and global runtime registrations |
| [`backend/dates.rs`](../../src/backend/dates.rs) | Native date helpers and civil-date/day-count conversions |
| [`backend/json.rs`](../../src/backend/json.rs) | `serde_json`-backed parsing, navigation, binding, rendering and table injection |
| [`backend/store.rs`](../../src/backend/store.rs) | File operations, terminal operations and standard-input conversion |
| [`backend/memory.rs`](../../src/backend/memory.rs) | Managed allocation, root frames, tracing collection and final heap cleanup |

Runtime exports use C ABI symbols named `xcx_*`. Generated code calls imported
helpers declared from `runtime::SIGNATURES`. The runtime lives in the same Rust crate
as the compiler and is distributed as a static library.

## Memory and I/O

The runtime has a thread-local managed heap. Managed objects can trace references
to other managed objects. Generated function frames publish roots; registered
global objects also contribute roots. Collection marks reachable objects and removes
unmarked entries from the heap.

Collection checks are emitted at statement safepoints and loop-related locations.
Temporary root slots belong to generated expression sites; roots are not a precise
last-use/liveness analysis. Some unreachable values can remain retained until a
slot is reused or the function frame exits. Byte accounting is approximate and
does not recursively measure every vector's backing storage.

Calls to `xcx_collect` skip collection until the allocation counter reaches 256 or
the tracked allocation-byte counter reaches 1,048,576 bytes. These are collection
threshold counters, not a hard heap-size limit. Normal launcher completion resets
global registrations and drops the managed heap through `xcx_runtime_finish`.
Top-level `HALT` returns through that launcher path, whereas `HALT` inside a user
function calls a process-exit helper and bypasses launcher finalization.

Output helpers write and flush lines immediately, supporting interactive prompts.
Checked runtime failures print `HALT:` diagnostics and terminate the process; this
does not guarantee interception of raw machine traps or internal panics. Store paths
are joined to the process working directory; absolute paths and `..` components are
rejected. This lexical check is not a filesystem sandbox or symlink confinement.
