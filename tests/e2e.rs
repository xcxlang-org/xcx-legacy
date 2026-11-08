mod common;

use std::process::{Command, Output, Stdio};
use std::str;

use common::fixture_path;

#[test]
fn missing_function_returns_are_reported_before_execution() {
    let output = run_program("invalid/missing-function-return.xcx");
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(str::from_utf8(&output.stderr).unwrap(), "error 1:1: function `message` can finish without returning a value\n");
    assert!(output.stdout.is_empty());
}

#[test]
fn returning_branches_and_loops_do_not_require_a_synthetic_return() {
    let output = run_program("valid/terminating-function-paths.xcx");
    assert!(output.status.success(), "{:?}", output);
    assert_eq!(str::from_utf8(&output.stdout).unwrap(), "first\nsecond\n42\n7\n");
}

#[test]
fn block_scopes_restore_the_outer_variable() {
    let output = run_program("valid/scoped-variables.xcx");
    assert!(output.status.success(), "{:?}", output);
    assert_eq!(str::from_utf8(&output.stdout).unwrap(), "2\n1\ninner\n1\n11\n12\n1\n1\n");
}

#[test]
fn same_named_local_tables_keep_distinct_runtime_schemas() {
    let output = run_program("valid/local-table-schemas.xcx");
    assert!(output.status.success(), "{:?}", output);
    assert_eq!(str::from_utf8(&output.stdout).unwrap(), "1\n1\n");
}

#[test]
fn map_get_works_without_an_intermediate_typed_variable() {
    let output = run_program("valid/map-get-expressions.xcx");
    assert!(output.status.success(), "{:?}", output);
    assert_eq!(str::from_utf8(&output.stdout).unwrap(), "42\n43\ntrue\nHello Alice\n42\n");
}

#[test]
fn table_filters_evaluate_date_constructors() {
    let output = run_program("valid/where-date-constructor.xcx");
    assert!(output.status.success(), "{:?}", output);
    assert_eq!(str::from_utf8(&output.stdout).unwrap(), "1\n2024-01-02\n");
}

#[test]
fn collection_values_survive_repeated_garbage_collection() {
    let output = run_program("valid/memory-lifetimes.xcx");
    assert!(output.status.success(), "{:?}", output);
    assert_eq!(str::from_utf8(&output.stdout).unwrap(), "item-1998\nitem-1999\nitem-1997\n");
}

#[test]
fn a_packaged_compiler_builds_programs_with_the_same_behavior_as_aot_run() {
    let workspace = std::env::temp_dir().join(format!("xcx-packaged-{}", std::process::id()));
    std::fs::create_dir_all(&workspace).expect("the package workspace must be creatable");
    let compiler = std::path::Path::new(env!("CARGO_BIN_EXE_xcx-legacy"));
    let compiler_name = compiler.file_name().expect("compiler filename");
    let packaged = workspace.join(compiler_name);
    std::fs::copy(compiler, &packaged).expect("copy compiler");
    let library = if cfg!(target_os = "windows") { "xcx.lib" } else { "libxcx.a" };
    std::fs::copy(compiler.parent().unwrap().join(library), workspace.join(library)).expect("copy runtime library");
    std::fs::copy(env!("XCX_LAUNCHER_OBJECT"), workspace.join("xcx_launcher.o")).expect("copy launcher");
    for (program, optimization) in [
        ("scoped-variables", "none"),
        ("local-table-schemas", "speed"),
        ("map-get-expressions", "size"),
        ("memory-lifetimes", "speed"),
    ] {
        let fixture = fixture_path(&format!("valid/{program}.xcx"));
        let built = Command::new(&packaged).arg("build").arg(&fixture)
            .args(["--release", "--opt-level", optimization]).current_dir(&workspace).output().expect("build with packaged compiler");
        assert!(built.status.success(), "{}", str::from_utf8(&built.stderr).unwrap());
        let native = run_built(&workspace, program);
        let aot = Command::new(&packaged).arg("run").arg(&fixture)
            .args(["--release", "--opt-level", optimization])
            .current_dir(&workspace).output().expect("run packaged compiler");
        assert!(native.status.success(), "{:?}", native);
        assert_eq!(native.status.code(), aot.status.code(), "{program}");
        assert_eq!(native.stdout, aot.stdout, "{program}");
        assert_eq!(native.stderr, aot.stderr, "{program}");
    }
    std::fs::remove_dir_all(&workspace).expect("the package workspace must be removable");
}

#[test]
fn an_interactive_prompt_is_visible_before_input_is_supplied() {
    use std::io::{BufRead, BufReader, Write};
    use std::sync::mpsc;
    use std::time::Duration;
    let mut child = Command::new(env!("CARGO_BIN_EXE_xcx-legacy"))
        .arg("run").arg(fixture_path("valid/interactive-prompt.xcx"))
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().expect("the interactive compiler must start");
    let stdout = child.stdout.take().expect("piped stdout");
    let (sender, receiver) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        let result = reader.read_line(&mut line).map(|_| line);
        sender.send(result).expect("the prompt receiver remains open");
        let mut rest = String::new();
        std::io::Read::read_to_string(&mut reader, &mut rest).expect("remaining output");
        rest
    });
    let prompt = receiver.recv_timeout(Duration::from_secs(10));
    child.stdin.take().expect("piped stdin").write_all(b"Alice\n").expect("input must be writable");
    let output = child.wait_with_output().expect("the program must exit");
    let rest = reader.join().expect("the reader must finish");
    assert_eq!(prompt.expect("the prompt must arrive before stdin").expect("read prompt"), "Enter your name:\n");
    assert!(output.status.success());
    assert_eq!(rest, "Alice\n");
}

fn run_xcx(arguments: &[&std::ffi::OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_xcx-legacy"))
        .args(arguments)
        .output()
        .expect("the xcx binary must be runnable")
}

fn run_program(relative_fixture: &str) -> Output {
    let path = fixture_path(relative_fixture);
    run_xcx(&[std::ffi::OsStr::new("run"), path.as_os_str()])
}

fn run_program_in(working_directory: &std::path::Path, program: &str) -> Output {
    run_in(working_directory, &["run", program])
}

fn run_in(working_directory: &std::path::Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_xcx-legacy"))
        .args(arguments)
        .current_dir(working_directory)
        .output()
        .expect("the xcx binary must be runnable")
}

fn built_binary_name(program: &str) -> String {
    if cfg!(target_os = "windows") {
        format!("{program}.exe")
    } else {
        program.to_string()
    }
}

fn run_built(working_directory: &std::path::Path, program: &str) -> Output {
    Command::new(working_directory.join(built_binary_name(program)))
        .current_dir(working_directory)
        .output()
        .expect("the built binary must be runnable")
}

fn run_program_with_stdin(relative_fixture: &str, input: &str) -> Output {
    let path = fixture_path(relative_fixture);
    let mut child = Command::new(env!("CARGO_BIN_EXE_xcx-legacy"))
        .arg("run")
        .arg(&path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the xcx binary must be spawnable");
    use std::io::Write;
    child
        .stdin
        .as_mut()
        .expect("stdin must be piped")
        .write_all(input.as_bytes())
        .expect("the input must be writable");
    child.wait_with_output().expect("the xcx binary must finish")
}

#[test]
fn run_succeeds_silently_on_a_valid_program() {
    let output = run_program("valid/empty.xcx");
    assert!(output.status.success(), "stderr: {}", str::from_utf8(&output.stderr).unwrap_or(""));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[test]
fn run_fails_with_a_positioned_error_on_an_invalid_program() {
    let output = run_program("invalid/unexpected-character.xcx");
    assert_eq!(output.status.code(), Some(1));
    let stderr = str::from_utf8(&output.stderr).unwrap_or("");
    assert!(stderr.contains("error 3:22: unexpected character `$`"), "{stderr}");
    assert!(stdout_is_empty(&output), "nothing may be printed to stdout on failure");
}

#[test]
fn run_reports_an_unreadable_file() {
    let output = run_xcx(&[std::ffi::OsStr::new("run"), std::ffi::OsStr::new("tests/fixtures/does-not-exist.xcx")]);
    assert_eq!(output.status.code(), Some(1));
    let stderr = str::from_utf8(&output.stderr).unwrap_or("");
    assert!(stderr.contains("cannot read"), "{stderr}");
}

#[test]
fn help_is_printed_to_stdout_on_request() {
    let output = run_xcx(&[std::ffi::OsStr::new("--help")]);
    assert!(output.status.success());
    let stdout = str::from_utf8(&output.stdout).unwrap_or("");
    assert!(stdout.contains("usage:"), "{stdout}");
    assert!(stdout.contains("AOT-compile"), "{stdout}");
}

#[test]
fn run_rejects_invalid_optimization_before_execution() {
    let output = Command::new(env!("CARGO_BIN_EXE_xcx-legacy"))
        .arg("run")
        .arg(fixture_path("run/arithmetic.xcx"))
        .args(["--opt-level", "invalid"])
        .output()
        .expect("the compiler must start");
    assert_eq!(output.status.code(), Some(2));
    let stderr = str::from_utf8(&output.stderr).unwrap();
    assert!(stderr.contains("invalid optimization `invalid`"), "{stderr}");
    assert!(output.stdout.is_empty(), "compilation must fail before execution");
}

#[test]
fn invocation_without_a_command_prints_usage_to_stderr_and_exits_with_2() {
    let output = run_xcx(&[]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = str::from_utf8(&output.stderr).unwrap_or("");
    assert!(stderr.contains("usage:"), "{stderr}");
    assert!(stdout_is_empty(&output));
}

fn stdout_is_empty(output: &Output) -> bool {
    output.stdout.is_empty()
}

#[test]
fn the_binary_executes_a_scalar_program_and_prints_its_output() {
    let output = run_program("run/arithmetic.xcx");
    assert!(output.status.success());
    let stdout = str::from_utf8(&output.stdout).unwrap_or("");
    assert_eq!(stdout, "13\n1\n-3\n48123456789\n31\n");
    assert!(stderr_is_empty(&output));
}

#[test]
fn the_binary_reports_programs_it_cannot_generate_yet() {
    let output = run_program("run-invalid/composite-literal.xcx");
    assert_eq!(output.status.code(), Some(1));
    let stderr = str::from_utf8(&output.stderr).unwrap_or("");
    assert!(stderr.contains("composite literals are generated in a later phase"), "{stderr}");
    assert!(stdout_is_empty(&output));
}

#[test]
fn the_binary_prints_a_date_in_iso_format() {
    let output = run_program("run/date-print.xcx");
    assert!(output.status.success());
    let stdout = str::from_utf8(&output.stdout).unwrap_or("");
    assert_eq!(stdout, "2024-01-01\n");
    assert!(stderr_is_empty(&output));
}

#[test]
fn the_binary_executes_dates_with_formats_and_arithmetic() {
    let output = run_program("run/dates.xcx");
    assert!(output.status.success(), "stderr: {}", str::from_utf8(&output.stderr).unwrap_or(""));
    let stdout = str::from_utf8(&output.stdout).unwrap_or("");
    assert_eq!(
        stdout,
        "2024\n12\n25\ntrue\ntrue\n2024-12-25\n25-12-2024\n12/25/2024\n12/25/2024\n2025-01-01\n2024-12-18\n-7\ntrue\ntrue\ntrue\ntrue\n"
    );
    assert!(stderr_is_empty(&output));
}

#[test]
fn the_binary_parses_and_binds_json() {
    let output = run_program("run/json.xcx");
    assert!(output.status.success(), "stderr: {}", str::from_utf8(&output.stderr).unwrap_or(""));
    let stdout = str::from_utf8(&output.stdout).unwrap_or("");
    assert_eq!(
        stdout,
        "2\ntrue\nfalse\nUser: John\n26\n91.5\ntrue\n2\noptional field does not exist\n2\n8080\ntrue\n"
    );
    assert!(stderr_is_empty(&output));
}

#[test]
fn the_binary_injects_json_into_a_table() {
    let output = run_program("run/json-inject.xcx");
    assert!(output.status.success(), "stderr: {}", str::from_utf8(&output.stderr).unwrap_or(""));
    let stdout = str::from_utf8(&output.stdout).unwrap_or("");
    assert_eq!(stdout, "2\n101\nAnna\n");
    assert!(stderr_is_empty(&output));
}

#[test]
fn the_binary_runs_the_temperature_monitoring_example() {
    let output = run_program("run/temperature-monitoring.xcx");
    assert!(output.status.success(), "stderr: {}", str::from_utf8(&output.stderr).unwrap_or(""));
    let stdout = str::from_utf8(&output.stdout).unwrap_or("");
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 5);
    assert_eq!(lines[0], "WARNING: Temperature 26.8 out of range in sensor S-002");
    assert_eq!(lines[1], "WARNING: Humidity 65.0% exceeds limit in sensor S-003");
    assert!(
        lines[2].starts_with("=== DAILY REPORT ") && lines[2].ends_with(" ==="),
        "{}",
        lines[2]
    );
    assert_eq!(lines[3], "Number of readings: 3");
    assert_eq!(lines[4], "Average temperature: 23.5°C");
}

#[test]
fn binding_a_missing_json_path_halts_with_exit_code_1() {
    let output = run_program("run-invalid/json-bind-missing.xcx");
    assert_eq!(output.status.code(), Some(1));
    let stderr = str::from_utf8(&output.stderr).unwrap_or("");
    assert!(stderr.contains("HALT: path `missing.path` does not exist in the json"), "{stderr}");
    let stdout = str::from_utf8(&output.stdout).unwrap_or("");
    assert_eq!(stdout, "");
}

#[test]
fn binding_a_null_json_value_halts_with_exit_code_1() {
    let output = run_program("run-invalid/json-bind-null.xcx");
    assert_eq!(output.status.code(), Some(1));
    let stderr = str::from_utf8(&output.stderr).unwrap_or("");
    assert!(stderr.contains("HALT: the value at path `value` is null"), "{stderr}");
    let stdout = str::from_utf8(&output.stdout).unwrap_or("");
    assert_eq!(stdout, "");
}

#[test]
fn an_invalid_runtime_date_halts_with_exit_code_1() {
    let output = run_program("run-invalid/date-runtime.xcx");
    assert_eq!(output.status.code(), Some(1));
    let stderr = str::from_utf8(&output.stderr).unwrap_or("");
    assert!(stderr.contains("HALT: invalid date: month 13 is outside 1-12"), "{stderr}");
    let stdout = str::from_utf8(&output.stdout).unwrap_or("");
    assert_eq!(stdout, "");
}

#[test]
fn the_binary_reads_input_into_variables() {
    let output = run_program_with_stdin("run/input.xcx", "Anna\n21\n1.5\ntrue\n");
    assert!(output.status.success(), "stderr: {}", str::from_utf8(&output.stderr).unwrap_or(""));
    let stdout = str::from_utf8(&output.stdout).unwrap_or("");
    assert_eq!(stdout, "Hello, Anna!\n42\n2.0\ntrue\n");
}

#[test]
fn the_binary_runs_terminal_commands() {
    let output = run_program("run/terminal.xcx");
    assert!(output.status.success(), "stderr: {}", str::from_utf8(&output.stderr).unwrap_or(""));
    let stdout = str::from_utf8(&output.stdout).unwrap_or("");
    assert_eq!(
        stdout,
        "before\n\u{1b}[2J\u{1b}[1;1H\nafter\navailable terminal commands:\n  !clean  clears the terminal screen\n  !exit   ends the XCX program\n  !help   displays available commands\n"
    );
}

#[test]
fn the_store_round_trips_files_inside_the_project_directory() {
    let workspace = std::env::temp_dir().join(format!("xcx-store-test-{}", std::process::id()));
    std::fs::create_dir_all(&workspace).expect("the temp workspace must be creatable");
    std::fs::copy(fixture_path("run/store.xcx"), workspace.join("store.xcx"))
        .expect("the fixture must be copyable");
    let output = run_program_in(&workspace, "store.xcx");
    let stdout = str::from_utf8(&output.stdout).unwrap_or("");
    let cleanup = std::fs::remove_dir_all(&workspace);
    assert!(output.status.success(), "stderr: {}", str::from_utf8(&output.stderr).unwrap_or(""));
    assert_eq!(stdout, "false\nfirst line\nfirst line\nsecond line\ntrue\n");
    cleanup.expect("the temp workspace must be removable");
}

#[test]
fn store_path_traversal_halts_with_exit_code_1() {
    let workspace = std::env::temp_dir().join(format!("xcx-store-traversal-{}", std::process::id()));
    std::fs::create_dir_all(&workspace).expect("the temp workspace must be creatable");
    std::fs::copy(
        fixture_path("run-invalid/store-traversal.xcx"),
        workspace.join("store-traversal.xcx"),
    )
    .expect("the fixture must be copyable");
    let output = run_program_in(&workspace, "store-traversal.xcx");
    let stderr = str::from_utf8(&output.stderr).unwrap_or("").to_string();
    let cleanup = std::fs::remove_dir_all(&workspace);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr.contains("HALT: store: `../outside.txt` leaves the project directory"),
        "{stderr}"
    );
    cleanup.expect("the temp workspace must be removable");
}

#[test]
fn a_multi_file_project_imports_configuration_utilities_and_json() {
    let project = fixture_path("project");
    let output = run_program_in(&project, "main.xcx");
    assert!(output.status.success(), "stderr: {}", str::from_utf8(&output.stderr).unwrap_or(""));
    let stdout = str::from_utf8(&output.stdout).unwrap_or("");
    assert_eq!(stdout, "xcx-demo\n100\n42\n8080\n");
}

#[test]
fn a_circular_import_is_rejected() {
    let project = fixture_path("project-cycle");
    let output = run_program_in(&project, "main.xcx");
    assert_eq!(output.status.code(), Some(1));
    let stderr = str::from_utf8(&output.stderr).unwrap_or("");
    assert!(stderr.contains("circular import detected"), "{stderr}");
    assert!(stdout_is_empty(&output));
}

#[test]
fn xcx_build_produces_a_standalone_binary_with_the_same_output_as_xcx_run() {
    let workspace = std::env::temp_dir().join(format!("xcx-build-test-{}", std::process::id()));
    std::fs::create_dir_all(&workspace).expect("the temp workspace must be creatable");
    std::fs::copy(fixture_path("run/tables.xcx"), workspace.join("tables.xcx"))
        .expect("the fixture must be copyable");

    let built = run_in(&workspace, &["build", "tables.xcx"]);
    assert!(
        built.status.success(),
        "stderr: {}",
        str::from_utf8(&built.stderr).unwrap_or("")
    );
    assert!(
        str::from_utf8(&built.stdout)
            .unwrap_or("")
            .contains(&format!("built `{}`", built_binary_name("tables"))),
        "{}",
        str::from_utf8(&built.stdout).unwrap_or("")
    );

    let binary = run_built(&workspace, "tables");
    let aot = run_in(&workspace, &["run", "tables.xcx"]);
    let cleanup = std::fs::remove_dir_all(&workspace);

    assert!(binary.status.success(), "the built binary must succeed");
    let binary_output = str::from_utf8(&binary.stdout).unwrap_or("");
    let aot_output = str::from_utf8(&aot.stdout).unwrap_or("");
    assert!(aot.status.success(), "{:?}", aot);
    assert_eq!(binary_output, aot_output);
    assert_eq!(
        binary_output,
        "Laptop\n2999.99\n3\nSmartphone\n1599.99\n2\n2\nLaptop\n0\n1\nLaptop\n2\nSmartphone\n"
    );
    cleanup.expect("the temp workspace must be removable");
}

#[test]
fn a_built_binary_reports_halt_failures_with_exit_code_1() {
    let workspace = std::env::temp_dir().join(format!("xcx-build-halt-{}", std::process::id()));
    std::fs::create_dir_all(&workspace).expect("the temp workspace must be creatable");
    std::fs::copy(
        fixture_path("run-invalid/table-row-out-of-range.xcx"),
        workspace.join("halt.xcx"),
    )
    .expect("the fixture must be copyable");

    let built = run_in(&workspace, &["build", "halt.xcx"]);
    assert!(built.status.success(), "the build must succeed");

    let binary = run_built(&workspace, "halt");
    let stderr = str::from_utf8(&binary.stderr).unwrap_or("").to_string();
    let stdout = str::from_utf8(&binary.stdout).unwrap_or("").to_string();
    let cleanup = std::fs::remove_dir_all(&workspace);

    assert_eq!(binary.status.code(), Some(1));
    assert!(stderr.contains("HALT: table row 5 does not exist"), "{stderr}");
    assert_eq!(stdout, "");
    cleanup.expect("the temp workspace must be removable");
}

#[test]
fn the_binary_executes_functions_with_recursion() {
    let output = run_program("run/functions.xcx");
    assert!(output.status.success());
    let stdout = str::from_utf8(&output.stdout).unwrap_or("");
    assert_eq!(stdout, "5! = 120\n5\nWelcome, XCX!\nHello, World\n");
    assert!(stderr_is_empty(&output));
}

#[test]
fn halt_inside_a_function_stops_the_whole_binary() {
    let output = run_program("run/halt-function.xcx");
    assert_eq!(output.status.code(), Some(0));
    let stdout = str::from_utf8(&output.stdout).unwrap_or("");
    assert_eq!(stdout, "before\nstopping\n");
}

#[test]
fn a_runtime_missing_map_key_halts_with_exit_code_1() {
    let output = run_program("run-invalid/missing-map-key-runtime.xcx");
    assert_eq!(output.status.code(), Some(1));
    let stderr = str::from_utf8(&output.stderr).unwrap_or("");
    assert!(stderr.contains("HALT: key \"not_exists\" does not exist in map"), "{stderr}");
    let stdout = str::from_utf8(&output.stdout).unwrap_or("");
    assert_eq!(stdout, "", "buffered output is flushed before the HALT message");
}

#[test]
fn the_binary_executes_sets_and_maps() {
    let output = run_program("run/maps.xcx");
    assert!(output.status.success());
    let stdout = str::from_utf8(&output.stdout).unwrap_or("");
    assert_eq!(
        stdout,
        "26\ntrue\n3\n{ \"John\" :: 26, \"Anna\" :: 30, \"Mary\" :: 28 }\n84\nOK\n2\n"
    );
}

#[test]
fn the_binary_executes_a_control_flow_program() {
    let output = run_program("run/loops.xcx");
    assert!(output.status.success());
    let stdout = str::from_utf8(&output.stdout).unwrap_or("");
    assert_eq!(stdout, "3\n15\n25\n3\n0\n");
    assert!(stderr_is_empty(&output));
}

#[test]
fn the_binary_executes_table_crud_and_where_filters() {
    let output = run_program("run/tables.xcx");
    assert!(output.status.success(), "stderr: {}", str::from_utf8(&output.stderr).unwrap_or(""));
    let stdout = str::from_utf8(&output.stdout).unwrap_or("");
    assert_eq!(
        stdout,
        "Laptop\n2999.99\n3\nSmartphone\n1599.99\n2\n2\nLaptop\n0\n1\nLaptop\n2\nSmartphone\n"
    );
    assert!(stderr_is_empty(&output));
}

#[test]
fn the_binary_joins_tables_with_aliased_columns() {
    let output = run_program("run/table-join.xcx");
    assert!(output.status.success(), "stderr: {}", str::from_utf8(&output.stderr).unwrap_or(""));
    let stdout = str::from_utf8(&output.stdout).unwrap_or("");
    assert_eq!(stdout, "3\nAdam\nLaptop\nPhone\n2\nAnna\n1\n1\n5000.0\n");
    assert!(stderr_is_empty(&output));
}

#[test]
fn the_binary_runs_the_library_example_with_global_tables() {
    let output = run_program("run/table-library.xcx");
    assert!(output.status.success(), "stderr: {}", str::from_utf8(&output.stderr).unwrap_or(""));
    let stdout = str::from_utf8(&output.stdout).unwrap_or("");
    assert_eq!(
        stdout,
        "Book added: War and Peace\nBook added: 1984\nBorrowed: War and Peace\nAvailable books: 1\n"
    );
    assert!(stderr_is_empty(&output));
}

#[test]
fn the_binary_compares_strings_lexicographically() {
    let output = run_program("run/string-comparisons.xcx");
    assert!(output.status.success(), "stderr: {}", str::from_utf8(&output.stderr).unwrap_or(""));
    let stdout = str::from_utf8(&output.stdout).unwrap_or("");
    assert_eq!(
        stdout,
        "false\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\nordered\n"
    );
    assert!(stderr_is_empty(&output));
}

#[test]
fn the_binary_runs_a_factory_function_returning_a_table() {
    let output = run_program("run/table-factory.xcx");
    assert!(output.status.success(), "stderr: {}", str::from_utf8(&output.stderr).unwrap_or(""));
    let stdout = str::from_utf8(&output.stdout).unwrap_or("");
    assert_eq!(stdout, "1\n1\n");
    assert!(stderr_is_empty(&output));
}

#[test]
fn the_binary_runs_a_procedure_with_an_inferred_table_return() {
    let output = run_program("run/table-inferred-return.xcx");
    assert!(output.status.success(), "stderr: {}", str::from_utf8(&output.stderr).unwrap_or(""));
    let stdout = str::from_utf8(&output.stdout).unwrap_or("");
    assert_eq!(stdout, "1\nPrepare report\n");
    assert!(stderr_is_empty(&output));
}

#[test]
fn indexing_a_missing_table_row_halts_with_exit_code_1() {
    let output = run_program("run-invalid/table-row-out-of-range.xcx");
    assert_eq!(output.status.code(), Some(1));
    let stderr = str::from_utf8(&output.stderr).unwrap_or("");
    assert!(stderr.contains("HALT: table row 5 does not exist"), "{stderr}");
    let stdout = str::from_utf8(&output.stdout).unwrap_or("");
    assert_eq!(stdout, "");
}

fn stderr_is_empty(output: &Output) -> bool {
    output.stderr.is_empty()
}
