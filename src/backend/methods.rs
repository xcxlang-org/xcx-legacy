use super::codegen::{val_description, Expectation, FunctionCodegen, Local, ShapeRef, Val};
use crate::diagnostics::Diagnostic;
use crate::frontend::ast::{Expr, Expression};
use cranelift::codegen::ir::{InstBuilder, MemFlagsData, StackSlotData, StackSlotKind, Value, types};

mod filter;
mod tables;

pub(super) use filter::table_where;
pub(super) use tables::{let_table, table_delete, table_insert, table_join, table_update};

pub(super) fn method_call(
    codegen: &mut FunctionCodegen,
    receiver_expr: &Expression,
    method: &str,
    arguments: &[Expression],
    expectation: Expectation,
    location: crate::diagnostics::Location,
) -> Result<Val, Diagnostic> {
    if let Expr::Ident(name) = &receiver_expr.node
        && !codegen.locals.contains_key(name)
        && crate::sema::environment::RESERVED_NAMES.contains(&name.as_str())
    {
        return namespace_call(codegen, name, method, arguments, location);
    }
    let receiver = codegen.expression_expect(receiver_expr, Expectation::Any)?;
    match (receiver, method) {
        (Val::Set(set), "is_empty") => {
            require_arity(arguments, 0, "is_empty", location)?;
            Ok(Val::Bool(codegen.call0(
                codegen.helpers.reference("xcx_set_is_empty"),
                &[set],
            )))
        }
        (Val::Set(set), "has") => {
            require_arity(arguments, 1, "has", location)?;
            let argument = codegen.expression(&arguments[0])?;
            set_has(codegen, set, argument, location)
        }
        (Val::Map(map, _), "insert") => {
            require_arity(arguments, 2, "insert", location)?;
            let key = codegen.expression(&arguments[0])?;
            let value = codegen.expression_expect(&arguments[1], Expectation::Any)?;
            let (key_a, key_b) = key_parts(codegen, &key, location)?;
            let (value_a, value_b, value_f) = value_parts(codegen, &value, location)?;
            let name = format!(
                "xcx_map_{}{}_insert",
                key_letter(&key, location)?,
                value_letter(&value, location)?
            );
            codegen
                .builder
                .ins()
                .call(codegen.helpers.reference(&name), &[map, key_a, key_b, value_a, value_b, value_f]);
            Ok(Val::Void)
        }
        (Val::Map(map, map_value), "get") => {
            require_arity(arguments, 1, "get", location)?;
            let key = codegen.expression(&arguments[0])?;
            let (key_a, key_b) = key_parts(codegen, &key, location)?;
            let value_letter = map_value.ok_or_else(|| Diagnostic::new(
                "the map expression has no statically known value type",
                location,
            ))?;
            let key_letter = key_letter(&key, location)?;
            let name = format!("xcx_map_{key_letter}{value_letter}_get");
            let out_slot = codegen
                .builder
                .create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 8, 3));
            let out_address = codegen.builder.ins().stack_addr(types::I64, out_slot, 0);
            let call = codegen.builder.ins().call(
                codegen.helpers.reference(&name),
                &[map, key_a, key_b, out_address],
            );
            let result = codegen.builder.inst_results(call)[0];
            if value_letter == 's' {
                let len = codegen
                    .builder
                    .ins()
                    .load(types::I64, MemFlagsData::new(), out_address, 0);
                return Ok(Val::Str { ptr: result, len });
            }
            Ok(match value_letter {
                'f' => Val::Float(result),
                'b' => Val::Bool(result),
                'c' => Val::Char(result),
                _ => Val::Int(result),
            })
        }
        (Val::Map(map, _), "has") => {
            require_arity(arguments, 1, "has", location)?;
            let key = codegen.expression(&arguments[0])?;
            let (key_a, key_b) = key_parts(codegen, &key, location)?;
            let name = format!("xcx_map_{}_has", key_letter(&key, location)?);
            Ok(Val::Bool(codegen.call0(
                codegen.helpers.reference(&name),
                &[map, key_a, key_b],
            )))
        }
        (Val::Map(map, _), "size") => {
            require_arity(arguments, 0, "size", location)?;
            Ok(Val::Int(codegen.call0(
                codegen.helpers.reference("xcx_map_size"),
                &[map],
            )))
        }
        (Val::Map(map, _), "show") => {
            require_arity(arguments, 0, "show", location)?;
            codegen
                .builder
                .ins()
                .call(codegen.helpers.reference("xcx_map_show"), &[map]);
            Ok(Val::Void)
        }
        (Val::Map(map, _), "keys") => {
            require_arity(arguments, 0, "keys", location)?;
            let name = match expectation {
                Expectation::Int => "xcx_map_i_keys",
                Expectation::Str => "xcx_map_s_keys",
                _ => {
                    return Err(Diagnostic::new(
                        "cannot determine the key type of `keys` — use it in a `for` loop with a typed variable",
                        location,
                    ));
                }
            };
            Ok(Val::Keys(codegen.call0(
                codegen.helpers.reference(name),
                &[map],
            )))
        }
        (Val::Str { ptr, len }, "length") => {
            require_arity(arguments, 0, "length", location)?;
            Ok(Val::Int(codegen.call0(
                codegen.helpers.reference("xcx_str_length"),
                &[ptr, len],
            )))
        }
        (Val::Date(days), "format") => match arguments.len() {
            0 => date_format(codegen, days, None, location),
            1 => {
                let value = codegen.expression_expect(&arguments[0], Expectation::Str)?;
                let Val::Str { ptr, len } = value else {
                    return Err(Diagnostic::new(
                        "the `format` template must be str",
                        location,
                    ));
                };
                date_format(codegen, days, Some((ptr, len)), location)
            }
            _ => Err(Diagnostic::new(
                "`format` expects 0 or 1 arguments",
                location,
            )),
        },
        (Val::Json(json), "count") => {
            require_arity(arguments, 0, "count", location)?;
            Ok(Val::Int(codegen.call0(
                codegen.helpers.reference("xcx_json_count"),
                &[json],
            )))
        }
        (Val::Json(json), "exists") => {
            require_arity(arguments, 1, "exists", location)?;
            let (path_ptr, path_len) = path_parts(codegen, &arguments[0], location)?;
            Ok(Val::Bool(codegen.call0(
                codegen.helpers.reference("xcx_json_exists"),
                &[json, path_ptr, path_len],
            )))
        }
        (Val::Json(json), "to_str") => {
            require_arity(arguments, 0, "to_str", location)?;
            let out_slot = codegen
                .builder
                .create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 8, 3));
            let out_address = codegen.builder.ins().stack_addr(types::I64, out_slot, 0);
            let ptr = codegen.call0(
                codegen.helpers.reference("xcx_json_to_str"),
                &[json, out_address],
            );
            let len = codegen
                .builder
                .ins()
                .load(types::I64, MemFlagsData::new(), out_address, 0);
            Ok(Val::Str { ptr, len })
        }
        (Val::Json(json), "bind") => {
            require_arity(arguments, 2, "bind", location)?;
            let (path_ptr, path_len) = path_parts(codegen, &arguments[0], location)?;
            json_bind(codegen, json, path_ptr, path_len, &arguments[1], location)
        }
        (Val::Json(_receiver_json), "inject") => {
            require_arity(arguments, 3, "inject", location)?;
            let value = codegen.expression_expect(&arguments[0], Expectation::Json)?;
            let Val::Json(target) = value else {
                return Err(Diagnostic::new(
                    "the first `inject` argument must be a json value",
                    location,
                ));
            };
            let map = codegen.expression_expect(&arguments[1], Expectation::Any)?;
            let Val::Map(mapping, _) = map else {
                return Err(Diagnostic::new(
                    "the second `inject` argument must be a map",
                    location,
                ));
            };
            let Some(table_name) = table_name_literal(&arguments[2]) else {
                return Err(Diagnostic::new(
                    "the third `inject` argument must be a table name literal",
                    location,
                ));
            };
            let (name_ptr, name_len) = codegen.constant_text(table_name);
            let table = codegen.call0(
                codegen.helpers.reference("xcx_global_table"),
                &[name_ptr, name_len],
            );
            codegen.builder.ins().call(
                codegen.helpers.reference("xcx_json_inject"),
                &[target, mapping, table],
            );
            Ok(Val::Void)
        }
        (Val::Table(table, shape), "insert") => {
            let Some(shape) = shape else {
                return Err(Diagnostic::new(
                    "cannot determine the shape of this table",
                    location,
                ));
            };
            table_insert(codegen, table, &shape, arguments, location)
        }
        (Val::Table(table, _), "update") => {
            table_update(codegen, table, arguments, location)
        }
        (Val::Table(table, _), "delete") => {
            table_delete(codegen, table, arguments, location)
        }
        (Val::Table(table, _), "count") => {
            require_arity(arguments, 0, "count", location)?;
            Ok(Val::Int(codegen.call0(
                codegen.helpers.reference("xcx_table_count"),
                &[table],
            )))
        }
        (Val::Table(table, Some(shape)), "where") => {
            require_arity(arguments, 1, "where", location)?;
            table_where(codegen, table, &shape, &arguments[0], location)
        }
        (Val::Table(table, shape), "join") => {
            table_join(codegen, table, &shape, arguments, location)
        }
        (Val::Table(..), method) => Err(Diagnostic::new(
            format!("tables have no method `{method}`"),
            location,
        )),
        (receiver, method) => Err(Diagnostic::new(
            format!(
                "method `{method}` is not generated for values of type {}",
                val_description(&receiver)
            ),
            location,
        )),
    }
}

pub(super) fn set_has(
    codegen: &mut FunctionCodegen,
    set: Value,
    argument: Val,
    location: crate::diagnostics::Location,
) -> Result<Val, Diagnostic> {
    let (name, arguments) = match &argument {
        Val::Int(value) => ("xcx_set_has_int", vec![set, *value]),
        Val::Float(value) => ("xcx_set_has_float", vec![set, *value]),
        Val::Str { ptr, len } => ("xcx_set_has_str", vec![set, *ptr, *len]),
        Val::Bool(value) => ("xcx_set_has_bool", vec![set, *value]),
        Val::Char(value) => ("xcx_set_has_char", vec![set, *value]),
        Val::Void | Val::Set(_) | Val::Map(..) | Val::Keys(_) | Val::Table(..) | Val::Row { .. }
        | Val::Date(_) | Val::Json(_) => {
            return Err(Diagnostic::new(
                "`has` needs an element of the set's type",
                location,
            ));
        }
    };
    Ok(Val::Bool(codegen.call0(codegen.helpers.reference(name), &arguments)))
}

fn require_arity(
    arguments: &[Expression],
    expected: usize,
    method: &str,
    location: crate::diagnostics::Location,
) -> Result<(), Diagnostic> {
    if arguments.len() != expected {
        return Err(Diagnostic::new(
            format!("`{method}` expects {expected} arguments, got {}", arguments.len()),
            location,
        ));
    }
    Ok(())
}

fn key_parts(
    codegen: &mut FunctionCodegen,
    value: &Val,
    location: crate::diagnostics::Location,
) -> Result<(Value, Value), Diagnostic> {
    match value {
        Val::Int(v) => Ok((*v, zero(codegen))),
        Val::Str { ptr, len } => Ok((*ptr, *len)),
        _ => Err(Diagnostic::new(
            "map keys must be int or str",
            location,
        )),
    }
}

fn value_parts(
    codegen: &mut FunctionCodegen,
    value: &Val,
    location: crate::diagnostics::Location,
) -> Result<(Value, Value, Value), Diagnostic> {
    match value {
        Val::Int(v) => Ok((*v, zero(codegen), zero_f(codegen))),
        Val::Float(v) => Ok((zero(codegen), zero(codegen), *v)),
        Val::Bool(v) => {
            let widened = codegen.builder.ins().uextend(types::I64, *v);
            Ok((widened, zero(codegen), zero_f(codegen)))
        }
        Val::Char(v) => {
            let widened = codegen.builder.ins().uextend(types::I64, *v);
            Ok((widened, zero(codegen), zero_f(codegen)))
        }
        Val::Str { ptr, len } => Ok((*ptr, *len, zero_f(codegen))),
        Val::Void | Val::Keys(_) | Val::Set(_) | Val::Map(..) | Val::Table(..) | Val::Row { .. }
        | Val::Date(_) | Val::Json(_) => {
            Err(Diagnostic::new(
                "map values must be int, float, boolean, char or str",
                location,
            ))
        }
    }
}

fn key_letter(value: &Val, location: crate::diagnostics::Location) -> Result<&'static str, Diagnostic> {
    match value {
        Val::Int(_) => Ok("i"),
        Val::Str { .. } => Ok("s"),
        _ => Err(Diagnostic::new("map keys must be int or str", location)),
    }
}

fn value_letter(value: &Val, location: crate::diagnostics::Location) -> Result<&'static str, Diagnostic> {
    match value {
        Val::Int(_) => Ok("i"),
        Val::Float(_) => Ok("f"),
        Val::Bool(_) => Ok("b"),
        Val::Char(_) => Ok("c"),
        Val::Str { .. } => Ok("s"),
        _ => Err(Diagnostic::new(
            "map values must be int, float, boolean, char or str",
            location,
        )),
    }
}

fn zero(codegen: &mut FunctionCodegen) -> Value {
    codegen.builder.ins().iconst(types::I64, 0)
}

fn zero_f(codegen: &mut FunctionCodegen) -> Value {
    codegen.builder.ins().f64const(0.0)
}

fn namespace_call(
    codegen: &mut FunctionCodegen,
    namespace: &str,
    method: &str,
    arguments: &[Expression],
    location: crate::diagnostics::Location,
) -> Result<Val, Diagnostic> {
    match (namespace, method) {
        ("date", "now") => {
            require_arity(arguments, 0, "now", location)?;
            Ok(Val::Date(codegen.call0(
                codegen.helpers.reference("xcx_date_now"),
                &[],
            )))
        }
        ("json", "parse") => {
            require_arity(arguments, 1, "parse", location)?;
            let value = codegen.expression_expect(&arguments[0], Expectation::Str)?;
            let Val::Str { ptr, len } = value else {
                return Err(Diagnostic::new(
                    "internal: the `json.parse` argument must be str",
                    location,
                ));
            };
            Ok(Val::Json(codegen.call0(
                codegen.helpers.reference("xcx_json_parse"),
                &[ptr, len],
            )))
        }
        ("store", "write") => {
            require_arity(arguments, 2, "write", location)?;
            let (path_ptr, path_len) = path_parts(codegen, &arguments[0], location)?;
            let data = codegen.expression_expect(&arguments[1], Expectation::Str)?;
            let Val::Str { ptr: data_ptr, len: data_len } = data else {
                return Err(Diagnostic::new(
                    "internal: the `store.write` content must be str",
                    location,
                ));
            };
            codegen.builder.ins().call(
                codegen.helpers.reference("xcx_store_write"),
                &[path_ptr, path_len, data_ptr, data_len],
            );
            Ok(Val::Void)
        }
        ("store", "read") => {
            require_arity(arguments, 1, "read", location)?;
            let (path_ptr, path_len) = path_parts(codegen, &arguments[0], location)?;
            let out_slot = codegen
                .builder
                .create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 8, 3));
            let out_address = codegen.builder.ins().stack_addr(types::I64, out_slot, 0);
            let ptr = codegen.call0(
                codegen.helpers.reference("xcx_store_read"),
                &[path_ptr, path_len, out_address],
            );
            let len = codegen
                .builder
                .ins()
                .load(types::I64, MemFlagsData::new(), out_address, 0);
            Ok(Val::Str { ptr, len })
        }
        ("store", "append") => {
            require_arity(arguments, 2, "append", location)?;
            let (path_ptr, path_len) = path_parts(codegen, &arguments[0], location)?;
            let data = codegen.expression_expect(&arguments[1], Expectation::Str)?;
            let Val::Str { ptr: data_ptr, len: data_len } = data else {
                return Err(Diagnostic::new(
                    "internal: the `store.append` content must be str",
                    location,
                ));
            };
            codegen.builder.ins().call(
                codegen.helpers.reference("xcx_store_append"),
                &[path_ptr, path_len, data_ptr, data_len],
            );
            Ok(Val::Void)
        }
        ("store", "exists") => {
            require_arity(arguments, 1, "exists", location)?;
            let (path_ptr, path_len) = path_parts(codegen, &arguments[0], location)?;
            Ok(Val::Bool(codegen.call0(
                codegen.helpers.reference("xcx_store_exists"),
                &[path_ptr, path_len],
            )))
        }
        (name, method) => Err(Diagnostic::new(
            format!("`{name}.{method}` is not generated yet"),
            location,
        )),
    }
}

fn date_format(
    codegen: &mut FunctionCodegen,
    days: Value,
    template: Option<(Value, Value)>,
    location: crate::diagnostics::Location,
) -> Result<Val, Diagnostic> {
    let (format_ptr, format_len) = match template {
        Some((ptr, len)) => (ptr, len),
        None => {
            let Val::Str { ptr, len } =
                codegen.string_literal(crate::sema::dates::DEFAULT_FORMAT)?
            else {
                unreachable!("string_literal yields a str");
            };
            (ptr, len)
        }
    };
    let out_slot = codegen
        .builder
        .create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 8, 3));
    let out_address = codegen.builder.ins().stack_addr(types::I64, out_slot, 0);
    let ptr = codegen.call0(
        codegen.helpers.reference("xcx_date_format"),
        &[days, format_ptr, format_len, out_address],
    );
    let len = codegen
        .builder
        .ins()
        .load(types::I64, MemFlagsData::new(), out_address, 0);
    let _ = location;
    Ok(Val::Str { ptr, len })
}

fn path_parts(
    codegen: &mut FunctionCodegen,
    expression: &Expression,
    location: crate::diagnostics::Location,
) -> Result<(Value, Value), Diagnostic> {
    let value = codegen.expression_expect(expression, Expectation::Str)?;
    let Val::Str { ptr, len } = value else {
        return Err(Diagnostic::new("the path must be str", location));
    };
    Ok((ptr, len))
}

fn table_name_literal(expression: &Expression) -> Option<&str> {
    match &expression.node {
        Expr::Str(text) => Some(text),
        _ => None,
    }
}

fn json_bind(
    codegen: &mut FunctionCodegen,
    json: Value,
    path_ptr: Value,
    path_len: Value,
    target: &Expression,
    location: crate::diagnostics::Location,
) -> Result<Val, Diagnostic> {
    let Expr::Ident(name) = &target.node else {
        return Err(Diagnostic::new(
            "internal: the bind target must be a variable",
            location,
        ));
    };
    let local = codegen.locals.get(name).cloned().ok_or_else(|| {
        Diagnostic::new(format!("undefined local `{name}`"), location)
    })?;
    match &local {
        Local::Scalar { ty, variable } => {
            let value = match *ty {
                types::I64 => codegen.call0(
                    codegen.helpers.reference("xcx_json_get_int"),
                    &[json, path_ptr, path_len],
                ),
                types::F64 => codegen.call0(
                    codegen.helpers.reference("xcx_json_get_float"),
                    &[json, path_ptr, path_len],
                ),
                _ => codegen.call0(
                    codegen.helpers.reference("xcx_json_get_bool"),
                    &[json, path_ptr, path_len],
                ),
            };
            codegen.builder.def_var(*variable, value);
        }
        Local::Str { ptr, len } => {
            let out_slot = codegen
                .builder
                .create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 8, 3));
            let out_address = codegen.builder.ins().stack_addr(types::I64, out_slot, 0);
            let value_ptr = codegen.call0(
                codegen.helpers.reference("xcx_json_get_str"),
                &[json, path_ptr, path_len, out_address],
            );
            let value_len = codegen
                .builder
                .ins()
                .load(types::I64, MemFlagsData::new(), out_address, 0);
            codegen.builder.def_var(*ptr, value_ptr);
            codegen.builder.def_var(*len, value_len);
        }
        other => {
            return Err(Diagnostic::new(
                format!(
                    "internal: `{name}` cannot be a bind target ({})",
                    match other {
                        Local::Days { .. } => "date",
                        Local::JsonBuffer { .. } => "json",
                        _ => "composite",
                    }
                ),
                location,
            ));
        }
    }
    Ok(Val::Void)
}
