use super::codegen::{Expectation, Flow, FunctionCodegen, Local, Val};
use super::collections;
use crate::diagnostics::Diagnostic;
use crate::frontend::ast::{SetElement, Statement, Stmt as AstStmt};
use cranelift::codegen::ir::{InstBuilder, Value, types};

pub(super) fn let_set(codegen: &mut FunctionCodegen, statement: &Statement) -> Result<Flow, Diagnostic> {
    let AstStmt::LetSet { name, designator, elements } = &statement.node else {
        unreachable!("let_set called with a non-set statement");
    };
    let designator_code = designator_code(designator)
        .expect("the parser validated the set designator");
    let element_expectation = element_expectation(designator_code);

    let builder_variable = codegen.builder.declare_var(types::I64);
    let zero = codegen.builder.ins().iconst(types::I32, designator_code as i64);
    let create = codegen
        .builder
        .ins()
        .call(codegen.helpers.reference("xcx_set_builder_new"), &[zero]);
    let set_builder = codegen.builder.inst_results(create)[0];
    codegen.root_pointer(set_builder);
    codegen.builder.def_var(builder_variable, set_builder);
    let builder_ptr = codegen.builder.use_var(builder_variable);

    for element in elements {
        match element {
            SetElement::Value(expression) => {
                let value = codegen.expression_expect(expression, element_expectation)?;
                add_element(codegen, builder_ptr, designator_code, value, statement)?;
            }
            SetElement::Range { start, end, step } => {
                add_range(codegen, builder_ptr, designator_code, start, end, step.as_ref(), statement)?;
            }
        }
    }

    let finish = codegen.builder.ins().call(
        codegen.helpers.reference("xcx_set_builder_finish"),
        &[builder_ptr],
    );
    let set_ptr = codegen.builder.inst_results(finish)[0];
    let variable = codegen.builder.declare_var(types::I64);
    codegen.builder.def_var(variable, set_ptr);
    if codegen.is_main {
        codegen.register_global_map(name, set_ptr);
    }
    codegen
        .locals
        .insert(name.clone(), Local::Opaque { kind: super::values::ObjectKind::Set, variable, map_value: None });
    Ok(Flow::FallThrough)
}

pub(super) fn map_literal(
    codegen: &mut FunctionCodegen,
    entries: &[crate::frontend::ast::MapEntry],
    location: crate::diagnostics::Location,
) -> Result<Val, Diagnostic> {
    let map = codegen
        .builder
        .ins()
        .call(codegen.helpers.reference("xcx_map_new"), &[]);
    let map_ptr = codegen.builder.inst_results(map)[0];
    codegen.root_pointer(map_ptr);

    let mut map_value = None;
    for entry in entries {
        let key = codegen.expression_expect(&entry.key, Expectation::Any)?;
        let value = codegen.expression_expect(&entry.value, Expectation::Any)?;
        let key_letter = value_letter(&key, location)?;
        let value_letter = value_letter(&value, location)?;
        map_value = Some(value_letter);
        let (key_a, key_b) = key_parts(codegen, &key, location)?;
        let (value_a, value_b, value_f) = value_parts(codegen, &value, location)?;
        let name = format!("xcx_map_{key_letter}{value_letter}_insert");
        codegen.builder.ins().call(
            codegen.helpers.reference(&name),
            &[map_ptr, key_a, key_b, value_a, value_b, value_f],
        );
    }
    Ok(Val::Map(map_ptr, map_value))
}

fn add_element(
    codegen: &mut FunctionCodegen,
    builder_ptr: Value,
    designator_code: i32,
    value: Val,
    statement: &Statement,
) -> Result<(), Diagnostic> {
    let name = match (&value, designator_code) {
        (Val::Int(_), collections::DESIGNATOR_NATURAL | collections::DESIGNATOR_INTEGER) => "xcx_set_add_int",
        (Val::Float(_), collections::DESIGNATOR_RATIONAL) => "xcx_set_add_float",
        (Val::Str { .. }, collections::DESIGNATOR_TEXT) => "xcx_set_add_str",
        (Val::Bool(_), collections::DESIGNATOR_BOOLEAN) => "xcx_set_add_bool",
        (Val::Char(_), collections::DESIGNATOR_CHARACTER) => "xcx_set_add_char",
        _ => {
            return Err(Diagnostic::new(
                "internal type mismatch building the set",
                statement.location,
            ));
        }
    };
    let arguments = flatten_value(&value);
    codegen
        .builder
        .ins()
        .call(codegen.helpers.reference(name), &prepend(builder_ptr, &arguments));
    Ok(())
}

fn add_range(
    codegen: &mut FunctionCodegen,
    builder_ptr: Value,
    designator_code: i32,
    start: &crate::frontend::ast::Expression,
    end: &crate::frontend::ast::Expression,
    step: Option<&crate::frontend::ast::Expression>,
    statement: &Statement,
) -> Result<(), Diagnostic> {
    let expectation = match designator_code {
        collections::DESIGNATOR_RATIONAL => Expectation::Float,
        _ => Expectation::Int,
    };
    let start_value = codegen.expression_expect(start, expectation)?;
    let end_value = codegen.expression_expect(end, expectation)?;
    let step_value = match step {
        Some(expression) => codegen.expression_expect(expression, expectation)?,
        None => match expectation {
            Expectation::Float => Val::Float(codegen.builder.ins().f64const(1.0)),
            _ => Val::Int(codegen.builder.ins().iconst(types::I64, 1)),
        },
    };
    let name = match designator_code {
        collections::DESIGNATOR_RATIONAL => "xcx_set_add_range_float",
        _ => "xcx_set_add_range_int",
    };
    let arguments = match (start_value, end_value, step_value) {
        (Val::Int(a), Val::Int(b), Val::Int(c)) => vec![a, b, c],
        (Val::Float(a), Val::Float(b), Val::Float(c)) => vec![a, b, c],
        _ => {
            return Err(Diagnostic::new(
                "internal type mismatch building the set range",
                statement.location,
            ));
        }
    };
    codegen
        .builder
        .ins()
        .call(codegen.helpers.reference(name), &prepend(builder_ptr, &arguments));
    Ok(())
}

fn flatten_value(value: &Val) -> Vec<Value> {
    match value {
        Val::Int(v) | Val::Char(v) | Val::Bool(v) => vec![*v],
        Val::Float(v) => vec![*v],
        Val::Str { ptr, len } => vec![*ptr, *len],
        _ => Vec::new(),
    }
}

fn prepend(builder_ptr: Value, arguments: &[Value]) -> Vec<Value> {
    let mut all = Vec::with_capacity(arguments.len() + 1);
    all.push(builder_ptr);
    all.extend_from_slice(arguments);
    all
}

fn designator_code(spelling: &str) -> Option<i32> {
    Some(match spelling {
        "N" => collections::DESIGNATOR_NATURAL,
        "Z" => collections::DESIGNATOR_INTEGER,
        "Q" => collections::DESIGNATOR_RATIONAL,
        "S" => collections::DESIGNATOR_TEXT,
        "B" => collections::DESIGNATOR_BOOLEAN,
        "C" => collections::DESIGNATOR_CHARACTER,
        _ => return None,
    })
}

fn element_expectation(designator_code: i32) -> Expectation {
    match designator_code {
        collections::DESIGNATOR_RATIONAL => Expectation::Float,
        collections::DESIGNATOR_TEXT => Expectation::Str,
        collections::DESIGNATOR_BOOLEAN => Expectation::Bool,
        collections::DESIGNATOR_CHARACTER => Expectation::Char,
        _ => Expectation::Int,
    }
}

fn value_letter(value: &Val, location: crate::diagnostics::Location) -> Result<char, Diagnostic> {
    match value {
        Val::Int(_) => Ok('i'),
        Val::Float(_) => Ok('f'),
        Val::Bool(_) => Ok('b'),
        Val::Char(_) => Ok('c'),
        Val::Str { .. } => Ok('s'),
        _ => Err(Diagnostic::new(
            "map keys and values must be int, float, boolean, char or str",
            location,
        )),
    }
}

fn key_parts(
    codegen: &mut FunctionCodegen,
    value: &Val,
    location: crate::diagnostics::Location,
) -> Result<(Value, Value), Diagnostic> {
    match value {
        Val::Int(v) => Ok((*v, codegen.builder.ins().iconst(types::I64, 0))),
        Val::Str { ptr, len } => Ok((*ptr, *len)),
        _ => Err(Diagnostic::new("map keys must be int or str", location)),
    }
}

fn value_parts(
    codegen: &mut FunctionCodegen,
    value: &Val,
    location: crate::diagnostics::Location,
) -> Result<(Value, Value, Value), Diagnostic> {
    match value {
        Val::Int(v) => Ok((*v, codegen.builder.ins().iconst(types::I64, 0), codegen.builder.ins().f64const(0.0))),
        Val::Float(v) => Ok((codegen.builder.ins().iconst(types::I64, 0), codegen.builder.ins().iconst(types::I64, 0), *v)),
        Val::Bool(v) => Ok((*v, codegen.builder.ins().iconst(types::I64, 0), codegen.builder.ins().f64const(0.0))),
        Val::Char(v) => Ok((*v, codegen.builder.ins().iconst(types::I64, 0), codegen.builder.ins().f64const(0.0))),
        Val::Str { ptr, len } => Ok((*ptr, *len, codegen.builder.ins().f64const(0.0))),
        _ => Err(Diagnostic::new(
            "map values must be int, float, boolean, char or str",
            location,
        )),
    }
}
