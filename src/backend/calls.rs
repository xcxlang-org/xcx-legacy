use super::codegen::{Expectation, Flow, FunctionCodegen, Val};
use super::values::{ReturnShape, SlotLayout};
use crate::diagnostics::Diagnostic;
use crate::frontend::ast::{Expression, Statement};
use cranelift::codegen::ir::{InstBuilder, MemFlagsData, StackSlotData, StackSlotKind, Value, types};

pub(super) fn call_expression(
    codegen: &mut FunctionCodegen,
    callee: &str,
    arguments: &[Expression],
    location: crate::diagnostics::Location,
) -> Result<Val, Diagnostic> {
    if callee == "date" {
        return date_constructor(codegen, arguments, location);
    }
    let Some(entry) = codegen.functions.get(callee).cloned() else {
        return Err(Diagnostic::new(
            format!("internal: unknown function `{callee}`"),
            location,
        ));
    };
    if arguments.len() != entry.parameters.len() {
        return Err(Diagnostic::new(
            format!("internal arity mismatch calling `{callee}`"),
            location,
        ));
    }

    let mut machine_arguments: Vec<Value> = Vec::new();
    let mut result_slot_address = None;
    if entry.returns == ReturnShape::Str {
        let slot = codegen
            .builder
            .create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 16, 4));
        let slot_address = codegen.builder.ins().stack_addr(types::I64, slot, 0);
        machine_arguments.push(slot_address);
        result_slot_address = Some(slot_address);
    }

    for (slot, argument) in entry.parameters.iter().zip(arguments) {
        let value = codegen.expression_expect(argument, Expectation::Any)?;
        match slot {
            SlotLayout::Scalar(ty) => machine_arguments.push(scalar_argument(
                *ty,
                value,
                callee,
                location,
            )?),
            SlotLayout::Str => match value {
                Val::Str { ptr, len } => {
                    machine_arguments.push(ptr);
                    machine_arguments.push(len);
                }
                _ => {
                    return Err(Diagnostic::new(
                        format!("internal type mismatch passing str to `{callee}`"),
                        location,
                    ));
                }
            },
            SlotLayout::Opaque => match value {
                Val::Table(ptr, _) | Val::Map(ptr, _) => machine_arguments.push(ptr),
                Val::Set(ptr) => machine_arguments.push(ptr),
                _ => {
                    return Err(Diagnostic::new(
                        format!("internal type mismatch passing an opaque value to `{callee}`"),
                        location,
                    ));
                }
            },
            SlotLayout::Days => match value {
                Val::Date(days) => machine_arguments.push(days),
                _ => {
                    return Err(Diagnostic::new(
                        format!("internal type mismatch passing a date to `{callee}`"),
                        location,
                    ));
                }
            },
        }
    }

    let function = *codegen
        .references
        .get(callee)
        .expect("every declared function has a reference in the current builder");
    let call = codegen.builder.ins().call(function, &machine_arguments);

    match &entry.returns {
        ReturnShape::Void => Ok(Val::Void),
        ReturnShape::Scalar(ty) => {
            let result = codegen.builder.inst_results(call)[0];
            Ok(match *ty {
                types::F64 => Val::Float(result),
                types::I8 => Val::Bool(result),
                _ => Val::Int(result),
            })
        }
        ReturnShape::Opaque => Ok(Val::Table(
            codegen.builder.inst_results(call)[0],
            entry.table_shape.clone(),
        )),
        ReturnShape::Days => Ok(Val::Date(codegen.builder.inst_results(call)[0])),
        ReturnShape::Str => {
            let slot_address = result_slot_address.expect("a str return allocated a result slot");
            let flags = MemFlagsData::new();
            let ptr = codegen.builder.ins().load(types::I64, flags, slot_address, 0);
            let len = codegen.builder.ins().load(types::I64, flags, slot_address, 8);
            Ok(Val::Str { ptr, len })
        }
    }
}

fn scalar_argument(
    ty: types::Type,
    value: Val,
    callee: &str,
    location: crate::diagnostics::Location,
) -> Result<Value, Diagnostic> {
    match (ty, value) {
        (types::I64, Val::Int(v)) => Ok(v),
        (types::F64, Val::Float(v)) => Ok(v),
        (types::I8, Val::Bool(v)) => Ok(v),
        (types::I64, Val::Date(v)) => Ok(v),
        _ => Err(Diagnostic::new(
            format!("internal type mismatch passing an argument to `{callee}`"),
            location,
        )),
    }
}

fn date_constructor(
    codegen: &mut FunctionCodegen,
    arguments: &[Expression],
    location: crate::diagnostics::Location,
) -> Result<Val, Diagnostic> {
    if arguments.is_empty() || arguments.len() > 2 {
        return Err(Diagnostic::new(
            format!(
                "internal: `date()` takes an ISO date and an optional format, got {} arguments",
                arguments.len()
            ),
            location,
        ));
    }
    let mut text_parts = None;
    let mut format_parts = None;
    for (position, argument) in arguments.iter().enumerate() {
        let value = codegen.expression_expect(argument, Expectation::Str)?;
        let Val::Str { ptr, len } = value else {
            return Err(Diagnostic::new(
                format!("internal: the `date()` argument {} must be str", position + 1),
                location,
            ));
        };
        if position == 0 {
            text_parts = Some((ptr, len));
        } else {
            format_parts = Some((ptr, len));
        }
    }
    let (text_ptr, text_len) = text_parts.expect("the text argument is always collected");
    let days = match format_parts {
        Some((format_ptr, format_len)) => {
            codegen.call0(
                codegen.helpers.reference("xcx_date_parse_format"),
                &[text_ptr, text_len, format_ptr, format_len],
            )
        }
        None => codegen.call0(
            codegen.helpers.reference("xcx_date_parse"),
            &[text_ptr, text_len],
        ),
    };
    Ok(Val::Date(days))
}

pub(super) fn return_statement(
    codegen: &mut FunctionCodegen,
    value: Option<&Expression>,
    statement: &Statement,
) -> Result<Flow, Diagnostic> {
    match codegen.return_shape.clone() {
        ReturnShape::Void => {
            codegen.leave_frame();
            codegen.builder.ins().return_(&[]);
        }
        ReturnShape::Scalar(ty) => {
            let Some(value) = value else {
                return Err(Diagnostic::new(
                    "internal: a typed function must return a value",
                    statement.location,
                ));
            };
            let produced = codegen.expression(value)?;
            let returned = match (ty, produced) {
                (types::I64, Val::Int(v)) => v,
                (types::F64, Val::Float(v)) => v,
                (types::I8, Val::Bool(v)) => v,
                (types::I64, Val::Date(v)) => v,
                _ => {
                    return Err(Diagnostic::new(
                        "internal type mismatch in the return value",
                        statement.location,
                    ));
                }
            };
            codegen.leave_frame();
            codegen.builder.ins().return_(&[returned]);
        }
        ReturnShape::Str => {
            let Some(value) = value else {
                return Err(Diagnostic::new(
                    "internal: a str-returning function must return a value",
                    statement.location,
                ));
            };
            let Val::Str { ptr, len } = codegen.expression(value)? else {
                return Err(Diagnostic::new(
                    "internal type mismatch in the returned string",
                    statement.location,
                ));
            };
            let out_pointer = codegen
                .out_ptr
                .expect("a str-returning function receives an out pointer");
            let out = codegen.builder.use_var(out_pointer);
            let flags = MemFlagsData::new();
            codegen.builder.ins().store(flags, ptr, out, 0);
            codegen.builder.ins().store(flags, len, out, 8);
            codegen.leave_frame();
            codegen.builder.ins().return_(&[]);
        }
        ReturnShape::Days => {
            let Some(value) = value else {
                return Err(Diagnostic::new(
                    "internal: a date-returning function must return a value",
                    statement.location,
                ));
            };
            let Val::Date(days) = codegen.expression(value)? else {
                return Err(Diagnostic::new(
                    "internal type mismatch in the returned date",
                    statement.location,
                ));
            };
            codegen.leave_frame();
            codegen.builder.ins().return_(&[days]);
        }
        ReturnShape::Opaque => {
            let Some(value) = value else {
                return Err(Diagnostic::new(
                    "internal: a table-returning function must return a value",
                    statement.location,
                ));
            };
            let Val::Table(ptr, _) = codegen.expression(value)? else {
                return Err(Diagnostic::new(
                    "internal type mismatch in the returned table",
                    statement.location,
                ));
            };
            codegen.leave_frame();
            codegen.builder.ins().return_(&[ptr]);
        }
    }
    Ok(Flow::Terminated)
}
