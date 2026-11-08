# Semantic analysis

## Modules

| File | Responsibility |
| --- | --- |
| [`sema.rs`](../../src/sema.rs) | Own `Checker`, collect function signatures, analyze statements and function bodies, return inferred function types |
| [`sema/environment.rs`](../../src/sema/environment.rs) | Scope stacks, variable/function/table declarations and declaration lookup |
| [`sema/types.rs`](../../src/sema/types.rs) | Semantic types, assignability, operator rules and readable type descriptions |
| [`sema/statements.rs`](../../src/sema/statements.rs) | Declaration, assignment, branch, loop and return checks |
| [`sema/expressions.rs`](../../src/sema/expressions.rs) | Expression typing, calls, fields, indexing and collection expressions |
| [`sema/builtins.rs`](../../src/sema/builtins.rs) | Built-in namespace and operation validation |
| [`sema/dates.rs`](../../src/sema/dates.rs) | Shared date parsing, validation and formatting rules |
| [`sema/json.rs`](../../src/sema/json.rs) | JSON path/type inspection used for statically known JSON values |
| [`sema/flow.rs`](../../src/sema/flow.rs) | Conservative analysis of fallthrough, empty returns, breaks and termination |

## Analysis sequence

`analyze` creates an environment, opens a scope, collects top-level function
signatures, then checks the program. Function bodies are checked in isolated
environments derived from the surrounding environment, with parameter scope and
function/loop state restored afterward. Isolation copies function declarations,
named collection values and registered table metadata, but creates fresh scalar
scopes. A function cannot capture an outer scalar variable or constant. This is not
general lexical closure support. Table-filter callbacks have separate capture
lowering in the backend.

Scopes permit shadowing in nested blocks while rejecting duplicate declarations in
the same scope. Entries carry mutability and semantic type information. Some entries
also carry literal map-key or JSON information, allowing certain mistakes to be
reported before execution. Dynamic values still require runtime checks. Literal
metadata is not a mutation-aware dataflow analysis: for example, an initial map's
known keys are not updated by `insert`, and a subsequent valid literal-key `get`
can be rejected. Known JSON information can likewise become stale after assignment.

The checker validates assignments, conditions, loop control, function arguments and
return values. It implements the language's conversion rules rather than applying
unrestricted numeric coercion. Table schemas and map value types participate in
expression checking. In particular, a direct integer literal can initialize a
declared float, but the checker does not apply that conversion uniformly to
assignments, function arguments and returns. Table assignability accepts different
schemas, and set assignability accepts different designators; operation-specific
checks provide additional restrictions. There is no definite-assignment pass.

## Return inference and control flow

`Analysis.function_returns` records inferred function-return types for the backend.
Inferred returns must be consistent. Table-return consistency compares column
schemas; other inferred returns use assignability checks in both directions.

Signatures are collected before checking, but inferred return results are filled
while functions are checked in source order. There is no fixed-point inference
across the call graph: a later function's unannotated return type may not yet be
available to an earlier caller. A declared signature and an inferred signature are
therefore not interchangeable in all cases.

`flow.rs` identifies whether a function can reach its end and whether reachable paths
contain a return without a value. Required value-returning functions are rejected
when a path can finish without producing a value. Explicit program termination and
certain endless loops are accounted for.

The endless-loop rule is syntactic: a literal `true` condition without a reachable
break. It is not a general termination proof or constant-expression evaluator.

## Boundary with code generation

Semantic acceptance does not guarantee that the backend supports every AST form.
For example, the array-style composite literal in
[`run-invalid/composite-literal.xcx`](../../tests/fixtures/run-invalid/composite-literal.xcx)
is rejected by code generation. Function parameter and return layouts are further
restricted by `backend/values.rs` and `backend/functions.rs`.

For example, the checker can infer a `date` return, but the backend's inferred-return
mapping omits that type. An explicitly annotated `date` return with a parameter
works; the corresponding unannotated function fails during backend declaration.
Similarly, semantic JSON binding accepts an integer JSON number as numeric for a
float target, but the runtime float accessor requires a float-classified JSON value
and rejects that integer with `HALT:`.

The checker does not produce a rewritten, fully typed AST. Backend code still
selects machine representations and generates operation-specific diagnostics.
