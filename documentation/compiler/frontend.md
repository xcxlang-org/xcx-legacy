# Frontend and diagnostics

## Modules

| File | Responsibility |
| --- | --- |
| [`frontend.rs`](../../src/frontend.rs) | Declare frontend modules |
| [`frontend/token.rs`](../../src/frontend/token.rs) | Token kinds and source positions |
| [`frontend/lexer.rs`](../../src/frontend/lexer.rs) | Scan identifiers, keywords, literals, operators, comments and raw blocks |
| [`frontend/ast.rs`](../../src/frontend/ast.rs) | Spanned AST nodes, declarations, expressions, types and operators |
| [`frontend/parser.rs`](../../src/frontend/parser.rs) | Parser state, token access and program/statement dispatch |
| [`frontend/parser/declarations.rs`](../../src/frontend/parser/declarations.rs) | Parse declarations, function signatures, imports, sets and tables |
| [`frontend/parser/expressions.rs`](../../src/frontend/parser/expressions.rs) | Parse expression precedence, operators, literals, calls and postfix chains |
| [`frontend/parser/flow.rs`](../../src/frontend/parser/flow.rs) | Parse branches, loops and flow-control statements |
| [`frontend/imports.rs`](../../src/frontend/imports.rs) | Read imported files, expand source imports and detect import cycles |
| [`diagnostics.rs`](../../src/diagnostics.rs) | Shared `Location` and `Diagnostic` types and diagnostic formatting |

The parser is handwritten and recursive. Expression parsing uses explicit precedence
levels. The AST uses `Spanned<T>` with line/column locations; both programs and blocks
are vectors of statements. The lexer checks literal syntax and integer literal
range and reports errors at the relevant source location.

Identifiers are ASCII letters, digits and underscores, with a letter or underscore
first. String contents can contain Unicode. Lexer columns count characters, whereas
runtime string lengths count UTF-8 bytes. Integer tokens are parsed as positive
`i64` values before unary minus is applied; consequently the source literal
`-9223372036854775808` is rejected even though that value fits the runtime integer.

## Parsing does not imply implementation

The table parser consumes expressions inside `rows = [...]` but discards them.
`Stmt::LetTable` retains the columns and no initial rows, so even `rows = [[42]]`
creates an empty table. Use runtime insertion to populate it. This is an
implementation gap, not validation of those row values against the columns.

Array expressions have AST nodes, but are not general runtime values. The backend
handles them in specific contexts such as `table.update(index, [values])`; printing
an array directly is unsupported.

An explicit function return annotation is parsed only inside the parameter-list
branch. A header such as `(answer -> int)` with no parameters fails to parse;
zero-argument functions without that annotation can use return inference.

## Import behavior

The compiler library accepts source text rather than a source filename. Consequently,
the first import-expansion base is `.`: root imports resolve relative to the current
working directory. Pass the appropriate working directory when invoking a project
with imports. The driver does not infer that directory from the input path.

Import expansion visits the top-level statement vector of each source file.
Imported source files are parsed recursively and flattened into that vector. Imports
in those imported files resolve relative to the importing file's canonical parent
directory. Canonical paths in the active recursion stack detect cycles.

An `import` inside an `if`, loop or function body is parsed but not expanded.
Semantic analysis and code generation ignore the remaining import statement, so
such an import silently does nothing, even when the referenced file is missing.

This is source expansion, not separate module compilation. There is no import cache
or namespace isolation; importing the same declarations repeatedly can produce
duplicate-definition errors. The non-JSON import path does not implement an alias
namespace.

An import ending in `.json` requires an alias. Its contents become a string constant
under that alias; expansion itself does not parse the file into a JSON runtime
object. JSON parsing and binding are handled by the language operations that consume
the string.

## Diagnostics

`Diagnostic` contains a message and `Location { line, column }`. Its display format
is `error <line>:<column>: <message>`. Imported parse errors add the imported path to
the message. There is no separate source-file ID or source-span database.

Most frontend and semantic operations return at the first propagated error. The
current frontend does not provide multi-error recovery. Compiler setup, linker and
execution failures can use location `0:0` because they are not tied to an AST node.
Runtime helpers use a separate `HALT:` reporting path for their checked errors.
This does not cover every possible failure: machine traps and internal compiler
panics are not converted universally into those diagnostics.
