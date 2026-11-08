use super::{dates, expressions, types, Checker};
use crate::diagnostics::{Diagnostic, Location};
use crate::frontend::ast::Expr;
use crate::frontend::ast::Expression;
use crate::sema::environment::VarEntry;
use crate::sema::types::{BuiltinNamespace, ColumnShape, SemaType, TableType};

const MUTATING_METHODS: [&str; 3] = ["insert", "update", "delete"];

pub(super) fn method_call_type(
    checker: &mut Checker,
    receiver: &Expression,
    method: &str,
    arguments: &[Expression],
    location: Location,
) -> Result<SemaType, Diagnostic> {
    let receiver_type = checker.expression_type(receiver)?;
    guard_constant_receiver(checker, receiver, &receiver_type, method, location)?;

    match receiver_type {
        SemaType::Namespace(namespace) => {
            namespace_method(checker, namespace, method, arguments, location)
        }
        SemaType::Set(_) => set_method(checker, method, arguments, location),
        SemaType::Map { key, value } => {
            map_method(checker, receiver, key, value, method, arguments, location)
        }
        SemaType::Table(table) => {
            table_method(checker, &table, method, arguments, location)
        }
        SemaType::Str => str_method(method, arguments, location),
        SemaType::Date => date_method(checker, method, arguments, location),
        SemaType::Json => json_method(checker, receiver, method, arguments, location),
        other => Err(Diagnostic::new(
            format!(
                "type {} has no method `{method}`",
                types::describe(&other)
            ),
            location,
        )),
    }
}

fn guard_constant_receiver(
    checker: &mut Checker,
    receiver: &Expression,
    receiver_type: &SemaType,
    method: &str,
    location: Location,
) -> Result<(), Diagnostic> {
    let Expr::Ident(name) = &receiver.node else {
        return Ok(());
    };
    if !MUTATING_METHODS.contains(&method) {
        return Ok(());
    }
    if !matches!(
        receiver_type,
        SemaType::Map { .. } | SemaType::Table(_)
    ) {
        return Ok(());
    }
    let entry = checker.environment.lookup(name);
    let Some(entry) = entry else {
        return Ok(());
    };
    if !entry.mutable {
        return Err(Diagnostic::new(
            format!("cannot modify constant `{name}`"),
            location,
        ));
    }
    Ok(())
}

fn namespace_method(
    checker: &mut Checker,
    namespace: BuiltinNamespace,
    method: &str,
    arguments: &[Expression],
    location: Location,
) -> Result<SemaType, Diagnostic> {
    match namespace {
        BuiltinNamespace::Store => match method {
            "write" | "append" => {
                require_arguments(arguments, 2, method, location)?;
                require_str_argument(checker, &arguments[0], location)?;
                require_str_argument(checker, &arguments[1], location)?;
                Ok(SemaType::Void)
            }
            "read" => {
                require_arguments(arguments, 1, method, location)?;
                require_str_argument(checker, &arguments[0], location)?;
                Ok(SemaType::Str)
            }
            "exists" => {
                require_arguments(arguments, 1, method, location)?;
                require_str_argument(checker, &arguments[0], location)?;
                Ok(SemaType::Bool)
            }
            _ => Err(unknown_namespace_method(namespace, method, location)),
        },
        BuiltinNamespace::Date => match method {
            "now" => {
                require_arguments(arguments, 0, method, location)?;
                Ok(SemaType::Date)
            }
            _ => Err(unknown_namespace_method(namespace, method, location)),
        },
        BuiltinNamespace::Json => match method {
            "parse" => {
                require_arguments(arguments, 1, method, location)?;
                require_str_argument(checker, &arguments[0], location)?;
                Ok(SemaType::Json)
            }
            _ => Err(unknown_namespace_method(namespace, method, location)),
        },
        BuiltinNamespace::Random => Err(unknown_namespace_method(namespace, method, location)),
    }
}

fn set_method(
    checker: &mut Checker,
    method: &str,
    arguments: &[Expression],
    location: Location,
) -> Result<SemaType, Diagnostic> {
    match method {
        "is_empty" => {
            require_arguments(arguments, 0, method, location)?;
            Ok(SemaType::Bool)
        }
        _ => {
            let _ = checker;
            Err(Diagnostic::new(
                format!("a set has no method `{method}`, expected `is_empty`"),
                location,
            ))
        }
    }
}

fn map_method(
    checker: &mut Checker,
    receiver: &Expression,
    key: Box<SemaType>,
    value: Box<SemaType>,
    method: &str,
    arguments: &[Expression],
    location: Location,
) -> Result<SemaType, Diagnostic> {
    match method {
        "insert" => {
            require_arguments(arguments, 2, method, location)?;
            require_assignable(checker, &key, &arguments[0], "the map key", location)?;
            require_assignable(checker, &value, &arguments[1], "the map value", location)?;
            Ok(SemaType::Void)
        }
        "get" => {
            require_arguments(arguments, 1, method, location)?;
            require_assignable(checker, &key, &arguments[0], "the map key", location)?;
            reject_missing_literal_key(checker, receiver, &arguments[0], location)?;
            Ok(*value)
        }
        "has" => {
            require_arguments(arguments, 1, method, location)?;
            require_assignable(checker, &key, &arguments[0], "the map key", location)?;
            Ok(SemaType::Bool)
        }
        "size" => {
            require_arguments(arguments, 0, method, location)?;
            Ok(SemaType::Int)
        }
        "show" => {
            require_arguments(arguments, 0, method, location)?;
            Ok(SemaType::Void)
        }
        "keys" => {
            require_arguments(arguments, 0, method, location)?;
            Ok(SemaType::Keys(key))
        }
        _ => Err(Diagnostic::new(
            format!(
                "a map has no method `{method}`, expected insert, get, has, size, show or keys"
            ),
            location,
        )),
    }
}

fn reject_missing_literal_key(
    checker: &mut Checker,
    receiver: &Expression,
    key_argument: &Expression,
    location: Location,
) -> Result<(), Diagnostic> {
    let Expr::Ident(name) = &receiver.node else {
        return Ok(());
    };
    let map_literal_keys = match checker.environment.lookup(name) {
        Some(entry) => entry.map_literal_keys.clone(),
        None => match checker.environment.value(name) {
            Some(entry) => entry.map_literal_keys.clone(),
            None => return Ok(()),
        },
    };
    let Some(keys) = map_literal_keys else {
        return Ok(());
    };
    let literal = match &key_argument.node {
        Expr::Str(text) => Some(crate::sema::environment::MapLiteralKey::Text(text.clone())),
        Expr::Int(value) => Some(crate::sema::environment::MapLiteralKey::Integer(*value)),
        _ => None,
    };
    let Some(literal) = literal else {
        return Ok(());
    };
    if !keys.contains(&literal) {
        return Err(Diagnostic::new(
            format!("key `{}` does not exist in map `{name}`", key_spelling(&literal)),
            location,
        ));
    }
    Ok(())
}

fn key_spelling(key: &crate::sema::environment::MapLiteralKey) -> String {
    match key {
        crate::sema::environment::MapLiteralKey::Text(text) => text.clone(),
        crate::sema::environment::MapLiteralKey::Integer(value) => value.to_string(),
    }
}

fn table_method(
    checker: &mut Checker,
    table: &TableType,
    method: &str,
    arguments: &[Expression],
    location: Location,
) -> Result<SemaType, Diagnostic> {
    let columns = &table.columns;
    match method {
        "insert" => {
            let expected_columns: Vec<&ColumnShape> =
                columns.iter().filter(|column| !column.auto).collect();
            if arguments.len() != expected_columns.len() {
                return Err(Diagnostic::new(
                    format!(
                        "`insert` expects {} values (one per non-auto column), got {}",
                        expected_columns.len(),
                        arguments.len()
                    ),
                    location,
                ));
            }
            for (position, (column, argument)) in
                expected_columns.iter().zip(arguments).enumerate()
            {
                let argument_type = checker.expression_type(argument)?;
                if !types::assignable(&column.ty, &argument_type) {
                    return Err(Diagnostic::new(
                        format!(
                            "argument {} of `insert` (column `{}`) must be {}, got {}",
                            position + 1,
                            column.name,
                            types::describe(&column.ty),
                            types::describe(&argument_type)
                        ),
                        location,
                    ));
                }
            }
            Ok(SemaType::Void)
        }
        "update" => {
            require_arguments(arguments, 2, method, location)?;
            let index_type = checker.expression_type(&arguments[0])?;
            if index_type != SemaType::Int {
                return Err(Diagnostic::new(
                    format!(
                        "the first `update` argument (the row index) must be int, got {}",
                        types::describe(&index_type)
                    ),
                    location,
                ));
            }
            let expected_columns: Vec<&ColumnShape> =
                columns.iter().filter(|column| !column.auto).collect();
            let SemaType::Array(values) = checker.expression_type(&arguments[1])? else {
                return Err(Diagnostic::new(
                    "the second `update` argument must be an array of row values",
                    location,
                ));
            };
            if values.len() != expected_columns.len() {
                return Err(Diagnostic::new(
                    format!(
                        "`update` expects {} values (one per non-auto column), got {}",
                        expected_columns.len(),
                        values.len()
                    ),
                    location,
                ));
            }
            for (position, (column, value_type)) in
                expected_columns.iter().zip(&values).enumerate()
            {
                if !types::assignable(&column.ty, value_type) {
                    return Err(Diagnostic::new(
                        format!(
                            "value {} of `update` (column `{}`) must be {}, got {}",
                            position + 1,
                            column.name,
                            types::describe(&column.ty),
                            types::describe(value_type)
                        ),
                        location,
                    ));
                }
            }
            Ok(SemaType::Void)
        }
        "delete" => {
            require_arguments(arguments, 1, method, location)?;
            let index_type = checker.expression_type(&arguments[0])?;
            if index_type != SemaType::Int {
                return Err(Diagnostic::new(
                    format!(
                        "`delete` expects an int row index, got {}",
                        types::describe(&index_type)
                    ),
                    location,
                ));
            }
            Ok(SemaType::Void)
        }
        "count" => {
            require_arguments(arguments, 0, method, location)?;
            Ok(SemaType::Int)
        }
        "where" => {
            require_arguments(arguments, 1, method, location)?;
            checker.environment.push_scope();
            for column in columns {
                let _ = checker.environment.declare(
                    &column.name,
                    VarEntry { ty: column.ty.clone(), mutable: true, map_literal_keys: None, json_literal: None },
                );
            }
            let condition_type = checker.expression_type(&arguments[0]);
            checker.environment.pop_scope();
            let condition_type = condition_type?;
            if condition_type != SemaType::Bool {
                return Err(Diagnostic::new(
                    format!(
                        "the `where` condition must be boolean, got {}",
                        types::describe(&condition_type)
                    ),
                    location,
                ));
            }
            Ok(SemaType::Table(TableType {
                name: table.name.clone(),
                columns: columns.clone(),
            }))
        }
        "join" => {
            require_arguments(arguments, 3, method, location)?;
            let other_type = checker.expression_type(&arguments[0])?;
            let SemaType::Table(other) = other_type else {
                return Err(Diagnostic::new(
                    format!(
                        "`join` expects a table, got {}",
                        types::describe(&other_type)
                    ),
                    location,
                ));
            };
            require_str_argument(checker, &arguments[1], location)?;
            require_str_argument(checker, &arguments[2], location)?;
            let Some(left_key) = expressions::string_literal(&arguments[1]) else {
                return Err(Diagnostic::new(
                    "the left join key must be a column name literal",
                    location,
                ));
            };
            let Some(right_key) = expressions::string_literal(&arguments[2]) else {
                return Err(Diagnostic::new(
                    "the right join key must be a column name literal",
                    location,
                ));
            };
            let Some(left_key_position) =
                columns.iter().position(|column| column.name == left_key)
            else {
                return Err(Diagnostic::new(
                    format!("unknown join key `{left_key}` in the left table"),
                    location,
                ));
            };
            let Some(right_key_position) =
                other.columns.iter().position(|column| column.name == right_key)
            else {
                return Err(Diagnostic::new(
                    format!("unknown join key `{right_key}` in the right table"),
                    location,
                ));
            };
            let merged = join_columns(table, &other, left_key_position, right_key_position, location)?;
            let name = match (&table.name, &other.name) {
                (Some(left), Some(right)) => Some(format!("{left}_{right}")),
                _ => None,
            };
            Ok(SemaType::Table(TableType { name, columns: merged }))
        }
        _ => Err(Diagnostic::new(
            format!(
                "a table has no method `{method}`, expected insert, update, delete, count, where or join"
            ),
            location,
        )),
    }
}

fn join_columns(
    left: &TableType,
    right: &TableType,
    left_key: usize,
    right_key: usize,
    location: Location,
) -> Result<Vec<ColumnShape>, Diagnostic> {
    let mut merged: Vec<ColumnShape> = Vec::new();
    for (position, column) in left.columns.iter().enumerate() {
        if position == left_key {
            merged.push(column.clone());
            continue;
        }
        if right.columns.iter().any(|other| other.name == column.name) {
            merged.push(alias_column(column, &left.name, location)?);
        } else {
            merged.push(column.clone());
        }
    }
    for (position, column) in right.columns.iter().enumerate() {
        if position == right_key {
            continue;
        }
        if left.columns.iter().any(|other| other.name == column.name) {
            merged.push(alias_column(column, &right.name, location)?);
        } else {
            merged.push(column.clone());
        }
    }
    Ok(merged)
}

fn alias_column(
    column: &ColumnShape,
    table_name: &Option<String>,
    location: Location,
) -> Result<ColumnShape, Diagnostic> {
    let Some(table_name) = table_name else {
        return Err(Diagnostic::new(
            format!(
                "cannot alias the column `{}` in `join` — colliding tables must be declared with `let (table)`",
                column.name
            ),
            location,
        ));
    };
    Ok(ColumnShape {
        name: format!("{table_name}_{}", column.name),
        ty: column.ty.clone(),
        auto: false,
    })
}

fn str_method(method: &str, arguments: &[Expression], location: Location) -> Result<SemaType, Diagnostic> {
    match method {
        "length" => {
            require_arguments(arguments, 0, method, location)?;
            Ok(SemaType::Int)
        }
        _ => Err(Diagnostic::new(
            format!("str has no method `{method}`, expected `length`"),
            location,
        )),
    }
}

fn date_method(
    checker: &mut Checker,
    method: &str,
    arguments: &[Expression],
    location: Location,
) -> Result<SemaType, Diagnostic> {
    match method {
        "format" => {
            if arguments.len() > 1 {
                return Err(Diagnostic::new(
                    "`format` expects 0 or 1 arguments",
                    location,
                ));
            }
            if let Some(template_expression) = arguments.first() {
                require_str_argument(checker, template_expression, location)?;
                if let Some(template) = expressions::string_literal(template_expression) {
                    dates::validate_format_template(template)
                        .map_err(|message| Diagnostic::new(message, location))?;
                }
            }
            Ok(SemaType::Str)
        }
        _ => Err(Diagnostic::new(
            format!("date has no method `{method}`, expected `format`"),
            location,
        )),
    }
}

fn json_method(
    checker: &mut Checker,
    receiver: &Expression,
    method: &str,
    arguments: &[Expression],
    location: Location,
) -> Result<SemaType, Diagnostic> {
    match method {
        "count" => {
            require_arguments(arguments, 0, method, location)?;
            Ok(SemaType::Int)
        }
        "exists" => {
            require_arguments(arguments, 1, method, location)?;
            require_str_argument(checker, &arguments[0], location)?;
            Ok(SemaType::Bool)
        }
        "bind" => {
            require_arguments(arguments, 2, method, location)?;
            require_str_argument(checker, &arguments[0], location)?;
            let Expr::Ident(target_name) = &arguments[1].node else {
                return Err(Diagnostic::new(
                    "the second `bind` argument must be a variable to write into",
                    location,
                ));
            };
            let target = checker.environment.lookup(target_name).ok_or_else(|| {
                Diagnostic::new(
                    format!("undefined variable `{target_name}`"),
                    location,
                )
            })?;
            if !target.mutable {
                return Err(Diagnostic::new(
                    format!("cannot modify constant `{target_name}`"),
                    location,
                ));
            }
            if !matches!(
                target.ty,
                SemaType::Int | SemaType::Float | SemaType::Bool | SemaType::Str
            ) {
                return Err(Diagnostic::new(
                    format!(
                        "cannot bind into a variable of type {}",
                        types::describe(&target.ty)
                    ),
                    location,
                ));
            }
            if let Some(static_value) = receiver_json_literal(checker, receiver) {
                check_static_bind(checker, static_value, &arguments[0], &target.ty, location)?;
            }
            Ok(SemaType::Void)
        }
        "inject" => {
            require_arguments(arguments, 3, method, location)?;
            checker.expression_type(&arguments[0])?;
            let mapping_type = checker.expression_type(&arguments[1])?;
            let SemaType::Map { key, value } = &mapping_type else {
                return Err(Diagnostic::new(
                    format!(
                        "the second `inject` argument must be a map, got {}",
                        types::describe(&mapping_type)
                    ),
                    location,
                ));
            };
            if **key != SemaType::Str || **value != SemaType::Str {
                return Err(Diagnostic::new(
                    "the `inject` mapping must be a map of str <-> str",
                    location,
                ));
            }
            require_str_argument(checker, &arguments[2], location)?;
            Ok(SemaType::Void)
        }
        "to_str" => {
            require_arguments(arguments, 0, method, location)?;
            Ok(SemaType::Str)
        }
        _ => Err(Diagnostic::new(
            format!(
                "json has no method `{method}`, expected count, exists, bind, inject or to_str"
            ),
            location,
        )),
    }
}

fn receiver_json_literal(checker: &Checker, receiver: &Expression) -> Option<serde_json::Value> {
    let Expr::Ident(name) = &receiver.node else {
        return None;
    };
    checker
        .environment
        .lookup(name)
        .and_then(|entry| entry.json_literal.clone())
}

fn check_static_bind(
    checker: &Checker,
    value: serde_json::Value,
    path: &Expression,
    target_type: &SemaType,
    location: Location,
) -> Result<(), Diagnostic> {
    let Some(path_text) = expressions::string_literal(path) else {
        return Ok(());
    };
    let Some(found) = crate::sema::json::navigate(&value, path_text) else {
        return Ok(());
    };
    if found.is_null() {
        return Ok(());
    }
    if !crate::sema::json::matches_type(found, target_type) {
        return Err(Diagnostic::new(
            format!(
                "the value at path `{path_text}` is {}, which does not match the type of the target variable ({})",
                crate::sema::json::describe(found),
                types::describe(target_type)
            ),
            location,
        ));
    }
    let _ = checker;
    Ok(())
}

fn require_arguments(
    arguments: &[Expression],
    expected: usize,
    method: &str,
    location: Location,
) -> Result<(), Diagnostic> {
    if arguments.len() != expected {
        return Err(Diagnostic::new(
            format!("`{method}` expects {expected} arguments, got {}", arguments.len()),
            location,
        ));
    }
    Ok(())
}

fn require_str_argument(
    checker: &mut Checker,
    argument: &Expression,
    location: Location,
) -> Result<(), Diagnostic> {
    let argument_type = checker.expression_type(argument)?;
    if argument_type != SemaType::Str {
        return Err(Diagnostic::new(
            format!("the argument must be str, got {}", types::describe(&argument_type)),
            location,
        ));
    }
    Ok(())
}

fn require_assignable(
    checker: &mut Checker,
    expected: &SemaType,
    argument: &Expression,
    what: &str,
    location: Location,
) -> Result<(), Diagnostic> {
    let argument_type = checker.expression_type(argument)?;
    if !types::assignable(expected, &argument_type) {
        return Err(Diagnostic::new(
            format!(
                "{what} must be {}, got {}",
                types::describe(expected),
                types::describe(&argument_type)
            ),
            location,
        ));
    }
    Ok(())
}

fn unknown_namespace_method(
    namespace: BuiltinNamespace,
    method: &str,
    location: Location,
) -> Diagnostic {
    Diagnostic::new(
        format!("builtin `{}` has no method `{method}`", namespace.name()),
        location,
    )
}
