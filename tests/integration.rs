mod common;

use common::fixture_source;
use xcx::compile;

#[test]
fn a_function_without_its_required_return_is_rejected() {
    let error = compile(&fixture_source("invalid/missing-function-return.xcx")).expect_err("missing return");
    assert_eq!(error.message, "function `message` can finish without returning a value");
    assert_eq!(error.location.line, 1);
}

#[test]
fn a_function_with_a_missing_returning_branch_is_rejected() {
    let error = compile(&fixture_source("invalid/partial-function-return.xcx")).expect_err("missing branch return");
    assert_eq!(error.message, "function `number` can finish without returning a value");
}

#[test]
fn an_inferred_value_return_cannot_mix_with_an_empty_return() {
    let error = compile(&fixture_source("invalid/inferred-empty-return.xcx")).expect_err("empty return");
    assert_eq!(error.message, "function `number` must return a value on every returning path");
    assert_eq!(error.location.line, 5);
}

#[test]
fn block_scopes_accept_shadowed_variables() {
    assert_eq!(compile(&fixture_source("valid/scoped-variables.xcx")), Ok(()));
}

#[test]
fn local_tables_can_use_the_same_name_for_different_schemas() {
    assert_eq!(compile(&fixture_source("valid/local-table-schemas.xcx")), Ok(()));
}

#[test]
fn map_values_are_typed_in_expressions() {
    assert_eq!(compile(&fixture_source("valid/map-get-expressions.xcx")), Ok(()));
}

#[test]
fn date_constructors_are_valid_in_table_filters() {
    assert_eq!(compile(&fixture_source("valid/where-date-constructor.xcx")), Ok(()));
}

#[test]
fn return_paths_can_terminate_through_branches_halt_or_an_endless_loop() {
    assert_eq!(compile(&fixture_source("valid/terminating-function-paths.xcx")), Ok(()));
}

#[test]
fn empty_fixture_compiles() {
    assert_eq!(compile(&fixture_source("valid/empty.xcx")), Ok(()));
}

#[test]
fn whitespace_only_fixture_compiles() {
    assert_eq!(compile(&fixture_source("valid/whitespace-only.xcx")), Ok(()));
}

#[test]
fn single_line_comments_fixture_compiles() {
    assert_eq!(compile(&fixture_source("valid/single-line-comments.xcx")), Ok(()));
}

#[test]
fn block_comment_fixture_compiles() {
    assert_eq!(compile(&fixture_source("valid/block-comment.xcx")), Ok(()));
}

#[test]
fn language_core_fixture_compiles() {
    assert_eq!(compile(&fixture_source("valid/language-core.xcx")), Ok(()));
}

#[test]
fn language_collections_fixture_compiles() {
    assert_eq!(compile(&fixture_source("valid/language-collections.xcx")), Ok(()));
}

#[test]
fn language_dates_json_fixture_compiles() {
    assert_eq!(compile(&fixture_source("valid/language-dates-json.xcx")), Ok(()));
}

#[test]
fn language_io_fixture_compiles() {
    assert_eq!(compile(&fixture_source("valid/language-io.xcx")), Ok(()));
}

#[test]
fn unexpected_character_is_reported_at_its_position() {
    let diagnostic =
        compile(&fixture_source("invalid/unexpected-character.xcx")).expect_err("fixture must be rejected");
    assert_eq!(diagnostic.location.line, 3);
    assert_eq!(diagnostic.location.column, 22);
    assert_eq!(diagnostic.message, "unexpected character `$`");
}

#[test]
fn unterminated_string_is_reported_at_its_opening_quote() {
    let diagnostic =
        compile(&fixture_source("invalid/unterminated-string.xcx")).expect_err("fixture must be rejected");
    assert_eq!(diagnostic.location.line, 1);
    assert_eq!(diagnostic.location.column, 15);
    assert_eq!(diagnostic.message, "unterminated string literal");
}

#[test]
fn unknown_escape_is_reported_at_its_backslash() {
    let diagnostic =
        compile(&fixture_source("invalid/unknown-escape.xcx")).expect_err("fixture must be rejected");
    assert_eq!(diagnostic.location.line, 1);
    assert_eq!(diagnostic.location.column, 17);
    assert_eq!(diagnostic.message, "unknown escape sequence `\\q`");
}

#[test]
fn integer_overflow_is_reported_at_the_literal() {
    let diagnostic =
        compile(&fixture_source("invalid/integer-overflow.xcx")).expect_err("fixture must be rejected");
    assert_eq!(diagnostic.location.line, 1);
    assert_eq!(diagnostic.location.column, 17);
    assert_eq!(
        diagnostic.message,
        "integer literal `99999999999999999999` does not fit in i64"
    );
}

#[test]
fn float_without_a_digit_after_the_point_is_reported_at_the_point() {
    let diagnostic =
        compile(&fixture_source("invalid/invalid-float.xcx")).expect_err("fixture must be rejected");
    assert_eq!(diagnostic.location.line, 1);
    assert_eq!(diagnostic.location.column, 18);
    assert_eq!(diagnostic.message, "expected a digit after the decimal point");
}

#[test]
fn unterminated_block_comment_is_reported_at_its_opening() {
    let diagnostic = compile(&fixture_source("invalid/unterminated-block-comment.xcx"))
        .expect_err("fixture must be rejected");
    assert_eq!(diagnostic.location.line, 2);
    assert_eq!(diagnostic.location.column, 1);
    assert_eq!(diagnostic.message, "unterminated multi-line comment");
}

#[test]
fn assigning_to_a_constant_is_rejected() {
    let diagnostic = compile(&fixture_source("invalid/constant-assignment.xcx"))
        .expect_err("fixture must be rejected");
    assert_eq!(diagnostic.location.line, 2);
    assert_eq!(diagnostic.message, "cannot modify constant `MAX_SIZE`");
}

#[test]
fn assigning_a_str_to_an_int_variable_is_rejected() {
    let diagnostic = compile(&fixture_source("invalid/type-mismatch.xcx"))
        .expect_err("fixture must be rejected");
    assert_eq!(diagnostic.location.line, 2);
    assert_eq!(
        diagnostic.message,
        "cannot assign str to int variable `number` — XCX performs no implicit type conversions"
    );
}

#[test]
fn union_of_different_set_types_is_rejected() {
    let diagnostic = compile(&fixture_source("invalid/mixed-set-union.xcx"))
        .expect_err("fixture must be rejected");
    assert_eq!(diagnostic.location.line, 3);
    assert_eq!(
        diagnostic.message,
        "set operations require two sets of the same type, got N and S"
    );
}

#[test]
fn reading_a_missing_map_key_is_rejected() {
    let diagnostic = compile(&fixture_source("invalid/missing-map-key.xcx"))
        .expect_err("fixture must be rejected");
    assert_eq!(diagnostic.location.line, 2);
    assert_eq!(diagnostic.message, "key `not_exists` does not exist in map `m`");
}

#[test]
fn an_impossible_date_is_rejected() {
    let diagnostic = compile(&fixture_source("invalid/invalid-date.xcx"))
        .expect_err("fixture must be rejected");
    assert_eq!(diagnostic.location.line, 1);
    assert_eq!(diagnostic.location.column, 16);
    assert_eq!(diagnostic.message, "invalid date: month 13 is outside 1-12");
}

#[test]
fn implicit_int_to_float_conversion_is_rejected() {
    let diagnostic = compile(&fixture_source("invalid/no-implicit-conversion.xcx"))
        .expect_err("fixture must be rejected");
    assert_eq!(diagnostic.location.line, 2);
    assert_eq!(diagnostic.message, "cannot initialize `x` of type float with int");
}

#[test]
fn an_int_literal_adapts_to_a_float_declaration() {
    assert_eq!(
        compile(&fixture_source("valid/float-from-int-literal.xcx")),
        Ok(())
    );
}

#[test]
fn an_undefined_variable_is_rejected() {
    let diagnostic = compile(&fixture_source("invalid/undefined-variable.xcx"))
        .expect_err("fixture must be rejected");
    assert_eq!(diagnostic.location.line, 1);
    assert_eq!(diagnostic.location.column, 4);
    assert_eq!(diagnostic.message, "undefined variable `missing`");
}

#[test]
fn table_insert_with_the_wrong_arity_is_rejected() {
    let diagnostic = compile(&fixture_source("invalid/table-insert-arity.xcx"))
        .expect_err("fixture must be rejected");
    assert_eq!(diagnostic.location.line, 9);
    assert_eq!(
        diagnostic.message,
        "`insert` expects 2 values (one per non-auto column), got 3"
    );
}

#[test]
fn a_table_initialized_with_a_non_table_is_rejected() {
    let diagnostic = compile(&fixture_source("invalid/table-init-not-a-table.xcx"))
        .expect_err("fixture must be rejected");
    assert_eq!(diagnostic.location.line, 8);
    assert_eq!(diagnostic.message, "cannot initialize `copy` of type table with int");
}

#[test]
fn joining_on_an_unknown_column_is_rejected() {
    let diagnostic = compile(&fixture_source("invalid/table-join-unknown-key.xcx"))
        .expect_err("fixture must be rejected");
    assert_eq!(diagnostic.location.line, 9);
    assert_eq!(diagnostic.message, "unknown join key `key` in the left table");
}

#[test]
fn a_json_bind_type_mismatch_is_rejected_at_compile_time() {
    let diagnostic = compile(&fixture_source("invalid/json-bind-type-mismatch.xcx"))
        .expect_err("fixture must be rejected");
    assert_eq!(diagnostic.location.line, 3);
    assert_eq!(
        diagnostic.message,
        "the value at path `name` is str, which does not match the type of the target variable (float)"
    );
}

#[test]
fn an_invalid_json_raw_block_is_rejected() {
    let diagnostic = compile(&fixture_source("invalid/json-raw-block.xcx"))
        .expect_err("fixture must be rejected");
    assert_eq!(diagnostic.location.line, 1);
    assert!(
        diagnostic.message.starts_with("invalid json in the raw block:"),
        "{}",
        diagnostic.message
    );
}

#[test]
fn an_inject_mapping_other_than_str_to_str_is_rejected() {
    let diagnostic = compile(&fixture_source("invalid/json-inject-mapping.xcx"))
        .expect_err("fixture must be rejected");
    assert_eq!(diagnostic.location.line, 7);
    assert_eq!(diagnostic.message, "the `inject` mapping must be a map of str <-> str");
}

#[test]
fn break_outside_of_a_loop_is_rejected() {
    let diagnostic = compile(&fixture_source("invalid/break-outside-loop.xcx"))
        .expect_err("fixture must be rejected");
    assert_eq!(diagnostic.location.line, 1);
    assert_eq!(diagnostic.message, "`break` outside of a loop");
}

#[test]
fn a_non_boolean_condition_is_rejected() {
    let diagnostic = compile(&fixture_source("invalid/condition-not-boolean.xcx"))
        .expect_err("fixture must be rejected");
    assert_eq!(diagnostic.location.line, 2);
    assert_eq!(diagnostic.message, "the `if` condition must be boolean, got int");
}
