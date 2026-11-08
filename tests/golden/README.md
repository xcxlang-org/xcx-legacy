# Golden Corpus

`tests/golden.rs` runs every `*.xcx` program in this directory through the compiler
(`xcx-legacy run`) and compares the result against the recorded artifacts:

- `<name>.out` — expected stdout, one assertion per line. A line ending with `*`
  matches by prefix only (used where the documented program prints the current date,
  e.g. `=== DAILY REPORT *`).
- `<name>.err` — expected stderr, compared exactly. Optional; when absent, stderr is
  not checked.
- `<name>.code` — expected process exit code. Optional; defaults to `0`.

The corpus covers the complete example programs from
[`documentation/language/15-examples.md`](../../documentation/language/15-examples.md):

| Case | Documentation section |
|------|----------------------|
| `15-1-library.xcx` | 15.1 Library Management System |
| `15-2-sales.xcx` | 15.2 Sales Data Analysis |
| `15-3-planning.xcx` | 15.3 Project Planning Application |
| `15-4-monitoring.xcx` | 15.4 Temperature Monitoring System |

Note on 15.3: the documented program uses the task id as a row index
(`complete_task(2)` on a two-row table), which halts — the golden record captures that
documented behavior (`table row 2 does not exist`, exit code 1) until the
documentation itself is amended.
