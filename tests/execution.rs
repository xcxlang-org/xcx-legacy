mod common;

use common::fixture_source;
use xcx::run;

#[test]
fn integer_arithmetic_runs_with_documented_semantics() {
    let output = run(&fixture_source("run/arithmetic.xcx")).expect("the program must execute");
    assert_eq!(output, "13\n1\n-3\n48123456789\n31\n");
}

#[test]
fn float_division_yields_floats_and_whole_floats_print_one_decimal() {
    let output = run(&fixture_source("run/floats.xcx")).expect("the program must execute");
    assert_eq!(output, "3.0\n2.5\n18.75\n-3.0\n");
}

#[test]
fn string_concatenation_accepts_every_scalar_on_both_sides() {
    let output = run(&fixture_source("run/strings.xcx")).expect("the program must execute");
    assert_eq!(output, "Hello from XCX 12\npi: 3.14159\non: true\n7 days\n");
}

#[test]
fn booleans_and_comparisons_print_true_and_false() {
    let output = run(&fixture_source("run/booleans.xcx")).expect("the program must execute");
    assert_eq!(output, "true\nfalse\ntrue\ntrue\nfalse\nfalse\ntrue\n");
}

#[test]
fn halt_stops_execution_at_the_exact_statement() {
    let output = run(&fixture_source("run/halt.xcx")).expect("the program must execute");
    assert_eq!(output, "before\n");
}

#[test]
fn if_elf_els_chains_select_the_matching_branch() {
    let output = run(&fixture_source("run/conditionals.xcx")).expect("the program must execute");
    assert_eq!(output, "Passed\nGrade: B\nAccess granted\nsigned in\n");
}

#[test]
fn loops_run_while_and_for_with_break_and_continue() {
    let output = run(&fixture_source("run/loops.xcx")).expect("the program must execute");
    assert_eq!(output, "3\n15\n25\n3\n0\n");
}

#[test]
fn nested_conditions_execute_the_inner_branch() {
    let output = run(&fixture_source("run/nested.xcx")).expect("the program must execute");
    assert_eq!(output, "Full access\n");
}

#[test]
fn strings_are_carried_across_loop_iterations() {
    let output = run(&fixture_source("run/string-loop.xcx")).expect("the program must execute");
    assert_eq!(output, "123\n");
}

#[test]
fn halt_inside_a_branch_stops_the_program() {
    let output = run(&fixture_source("run/halt-branch.xcx")).expect("the program must execute");
    assert_eq!(output, "hit\n");
}

#[test]
fn composite_literals_are_reported_as_not_generated_yet() {
    let diagnostic = run(&fixture_source("run-invalid/composite-literal.xcx"))
        .expect_err("composite literals are generated in a later phase");
    assert_eq!(
        diagnostic.message,
        "composite literals are generated in a later phase"
    );
}

#[test]
fn sets_build_with_ranges_enumeration_and_algebra() {
    let output = run(&fixture_source("run/sets.xcx")).expect("the program must execute");
    assert_eq!(
        output,
        "SetObject(SET_N: [2..10:2])\n\
         SetObject(SET_N: [1..8:1])\n\
         SetObject(SET_N: [4..5:1])\n\
         SetObject(SET_N: [1..3:1])\n\
         SetObject(SET_N: [1..3:1, 6..8:1])\n\
         red is in\n\
         zero is in\n\
         true\n\
         true\n"
    );
}

#[test]
fn maps_run_crud_size_show_and_key_iteration() {
    let output = run(&fixture_source("run/maps.xcx")).expect("the program must execute");
    assert_eq!(
        output,
        "26\ntrue\n3\n{ \"John\" :: 26, \"Anna\" :: 30, \"Mary\" :: 28 }\n84\nOK\n2\n"
    );
}

#[test]
fn functions_run_recursion_multi_parameter_procedures_and_str_returns() {
    let output = run(&fixture_source("run/functions.xcx")).expect("the program must execute");
    assert_eq!(output, "5! = 120\n5\nWelcome, XCX!\nHello, World\n");
}
