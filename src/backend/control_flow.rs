use super::codegen::{Expectation, Flow, FunctionCodegen, Local, Val};
use crate::diagnostics::Diagnostic;
use crate::frontend::ast::{Branch, Expression, Statement};
use cranelift::codegen::ir::condcodes::IntCC;
use cranelift::codegen::ir::{InstBuilder, MemFlagsData, StackSlotData, StackSlotKind, Value, types};

pub(super) fn if_statement(
    codegen: &mut FunctionCodegen,
    branches: &[Branch],
    otherwise: Option<&crate::frontend::ast::Block>,
) -> Result<Flow, Diagnostic> {
    let merge = codegen.builder.create_block();
    let mut falls_through = false;

    for branch in branches {
        let condition = codegen.bool_expression(&branch.condition)?;
        let then_block = codegen.builder.create_block();
        let next_block = codegen.builder.create_block();
        codegen
            .builder
            .ins()
            .brif(condition, then_block, &[], next_block, &[]);
        codegen.builder.switch_to_block(then_block);
        codegen.builder.seal_block(then_block);
        let flow = codegen.emit_body(&branch.body)?;
        if flow == Flow::FallThrough {
            falls_through = true;
            codegen.builder.ins().jump(merge, &[]);
        }
        codegen.builder.switch_to_block(next_block);
        codegen.builder.seal_block(next_block);
    }

    if let Some(body) = otherwise {
        let flow = codegen.emit_body(body)?;
        if flow == Flow::FallThrough {
            falls_through = true;
            codegen.builder.ins().jump(merge, &[]);
        }
    } else {
        falls_through = true;
        codegen.builder.ins().jump(merge, &[]);
    }

    codegen.builder.seal_block(merge);
    if falls_through {
        codegen.builder.switch_to_block(merge);
        Ok(Flow::FallThrough)
    } else {
        Ok(Flow::Terminated)
    }
}

pub(super) fn while_statement(
    codegen: &mut FunctionCodegen,
    condition: &Expression,
    body: &crate::frontend::ast::Block,
) -> Result<Flow, Diagnostic> {
    let endless = crate::sema::flow::is_endless_loop(condition, body);
    let header = codegen.builder.create_block();
    codegen.builder.ins().jump(header, &[]);
    codegen.builder.switch_to_block(header);

    let condition_value = codegen.bool_expression(condition)?;
    codegen.builder.ins().call(codegen.helpers.reference("xcx_collect"), &[]);
    let body_block = codegen.builder.create_block();
    let exit_block = codegen.builder.create_block();
    if endless {
        codegen.builder.ins().jump(body_block, &[]);
    } else {
        codegen.builder.ins().brif(condition_value, body_block, &[], exit_block, &[]);
    }
    codegen.builder.switch_to_block(body_block);
    codegen.builder.seal_block(body_block);

    codegen.loops.push((header, exit_block));
    let flow = codegen.emit_body(body)?;
    codegen.loops.pop();

    if flow == Flow::FallThrough {
        codegen.builder.ins().jump(header, &[]);
    }
    codegen.builder.seal_block(header);
    if endless {
        codegen.builder.seal_block(exit_block);
        return Ok(Flow::Terminated);
    }
    codegen.builder.switch_to_block(exit_block);
    codegen.builder.seal_block(exit_block);
    Ok(Flow::FallThrough)
}

pub(super) fn for_range_statement(
    codegen: &mut FunctionCodegen,
    variable: &str,
    start: &Expression,
    end: &Expression,
    body: &crate::frontend::ast::Block,
) -> Result<Flow, Diagnostic> {
    let start_value = codegen.expression(start)?;
    let end_value = codegen.expression(end)?;
    let (Val::Int(start_value), Val::Int(end_value)) = (start_value, end_value) else {
        return Err(Diagnostic::new(
            "internal type mismatch in the `for` range",
            crate::diagnostics::Location { line: 0, column: 0 },
        ));
    };

    let counter = codegen.builder.declare_var(types::I64);
    codegen.builder.def_var(counter, start_value);
    let end_slot = codegen.builder.declare_var(types::I64);
    codegen.builder.def_var(end_slot, end_value);

    let previous_local = codegen.locals.insert(
        variable.to_string(),
        Local::Scalar { ty: types::I64, variable: counter },
    );

    let header = codegen.builder.create_block();
    codegen.builder.ins().jump(header, &[]);
    codegen.builder.switch_to_block(header);

    let current = codegen.builder.use_var(counter);
    let last = codegen.builder.use_var(end_slot);
    let condition = codegen
        .builder
        .ins()
        .icmp(IntCC::SignedLessThanOrEqual, current, last);

    let body_block = codegen.builder.create_block();
    let step_block = codegen.builder.create_block();
    let exit_block = codegen.builder.create_block();
    codegen
        .builder
        .ins()
        .brif(condition, body_block, &[], exit_block, &[]);
    codegen.builder.switch_to_block(body_block);
    codegen.builder.seal_block(body_block);

    codegen.loops.push((step_block, exit_block));
    let flow = codegen.emit_body(body)?;
    codegen.loops.pop();

    if flow == Flow::FallThrough {
        codegen.builder.ins().jump(step_block, &[]);
    }
    codegen.builder.switch_to_block(step_block);
    codegen.builder.seal_block(step_block);
    let increment = one_i64(codegen);
    let current = codegen.builder.use_var(counter);
    let incremented = codegen.builder.ins().iadd(current, increment);
    codegen.builder.def_var(counter, incremented);
    codegen.builder.ins().jump(header, &[]);
    codegen.builder.seal_block(header);

    codegen.builder.switch_to_block(exit_block);
    codegen.builder.seal_block(exit_block);

    match previous_local {
        Some(local) => {
            codegen.locals.insert(variable.to_string(), local);
        }
        None => {
            codegen.locals.remove(variable);
        }
    }
    Ok(Flow::FallThrough)
}

pub(super) fn for_in_statement(
    codegen: &mut FunctionCodegen,
    variable_type: &crate::frontend::ast::Type,
    variable: &str,
    iterable: &Expression,
    body: &crate::frontend::ast::Block,
) -> Result<Flow, Diagnostic> {
    let expectation = codegen.expectation_from_type(variable_type);
    let list = match codegen.expression_expect(iterable, expectation)? {
        Val::Keys(list) => list,
        _ => {
            return Err(Diagnostic::new(
                "internal: the iterable must be map keys",
                crate::diagnostics::Location { line: 0, column: 0 },
            ));
        }
    };

    let index = codegen.builder.declare_var(types::I64);
    let zero_index = codegen.builder.ins().iconst(types::I64, 0);
    codegen.builder.def_var(index, zero_index);

    let previous_local = codegen.locals.get(variable).cloned();
    declare_loop_local(codegen, variable, expectation)?;

    let header = codegen.builder.create_block();
    codegen.builder.ins().jump(header, &[]);
    codegen.builder.switch_to_block(header);

    let current_index = codegen.builder.use_var(index);
    let length = codegen.call0(codegen.helpers.reference("xcx_keys_len"), &[list]);
    let condition = codegen
        .builder
        .ins()
        .icmp(IntCC::SignedLessThan, current_index, length);

    let body_block = codegen.builder.create_block();
    let step_block = codegen.builder.create_block();
    let exit_block = codegen.builder.create_block();
    codegen
        .builder
        .ins()
        .brif(condition, body_block, &[], exit_block, &[]);
    codegen.builder.switch_to_block(body_block);
    codegen.builder.seal_block(body_block);

    let element = match expectation {
        Expectation::Int => Some(codegen.call0(
            codegen.helpers.reference("xcx_keys_get_int"),
            &[list, current_index],
        )),
        Expectation::Str => {
            let slot = codegen
                .builder
                .create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 8, 3));
            let slot_address = codegen.builder.ins().stack_addr(types::I64, slot, 0);
            let ptr = codegen.call0(
                codegen.helpers.reference("xcx_keys_get_str"),
                &[list, current_index, slot_address],
            );
            let len = codegen
                .builder
                .ins()
                .load(types::I64, MemFlagsData::new(), slot_address, 0);
            match codegen.locals.get(variable).cloned() {
                Some(Local::Str { ptr: var_ptr, len: var_len }) => {
                    codegen.builder.def_var(var_ptr, ptr);
                    codegen.builder.def_var(var_len, len);
                }
                _ => unreachable!("the loop variable was declared as a str local"),
            }
            None
        }
        _ => {
            return Err(Diagnostic::new(
                "map keys can only be int or str",
                crate::diagnostics::Location { line: 0, column: 0 },
            ));
        }
    };
    if let Some(value) = element {
        match codegen.locals.get(variable).cloned() {
            Some(Local::Scalar { variable, .. }) => {
                codegen.builder.def_var(variable, value);
            }
            _ => unreachable!("the loop variable was declared as a scalar local"),
        }
    }

    codegen.loops.push((step_block, exit_block));
    let flow = codegen.emit_body(body)?;
    codegen.loops.pop();

    if flow == Flow::FallThrough {
        codegen.builder.ins().jump(step_block, &[]);
    }
    codegen.builder.switch_to_block(step_block);
    codegen.builder.seal_block(step_block);
    let increment = codegen.builder.ins().iconst(types::I64, 1);
    let current_index = codegen.builder.use_var(index);
    let advanced = codegen.builder.ins().iadd(current_index, increment);
    codegen.builder.def_var(index, advanced);
    codegen.builder.ins().jump(header, &[]);
    codegen.builder.seal_block(header);

    codegen.builder.switch_to_block(exit_block);
    codegen.builder.seal_block(exit_block);

    match previous_local {
        Some(local) => {
            codegen.locals.insert(variable.to_string(), local);
        }
        None => {
            codegen.locals.remove(variable);
        }
    }
    Ok(Flow::FallThrough)
}

fn declare_loop_local(
    codegen: &mut FunctionCodegen,
    variable: &str,
    expectation: Expectation,
) -> Result<(), Diagnostic> {
    let local = match expectation {
        Expectation::Int => Local::Scalar { ty: types::I64, variable: codegen.builder.declare_var(types::I64) },
        Expectation::Float => Local::Scalar { ty: types::F64, variable: codegen.builder.declare_var(types::F64) },
        Expectation::Bool => Local::Scalar { ty: types::I8, variable: codegen.builder.declare_var(types::I8) },
        Expectation::Char => Local::Scalar { ty: types::I32, variable: codegen.builder.declare_var(types::I32) },
        Expectation::Str => Local::Str {
            ptr: codegen.builder.declare_var(types::I64),
            len: codegen.builder.declare_var(types::I64),
        },
        Expectation::Any | Expectation::Date | Expectation::Json => {
            return Err(Diagnostic::new(
                "the `for … in` loop variable needs a type",
                crate::diagnostics::Location { line: 0, column: 0 },
            ));
        }
    };
    codegen.locals.insert(variable.to_string(), local);
    Ok(())
}

pub(super) fn break_statement(
    codegen: &mut FunctionCodegen,
    statement: &Statement,
) -> Result<Flow, Diagnostic> {
    let Some((_, exit_block)) = codegen.loops.last().copied() else {
        return Err(Diagnostic::new(
            "`break` outside of a loop",
            statement.location,
        ));
    };
    codegen.builder.ins().jump(exit_block, &[]);
    Ok(Flow::Terminated)
}

pub(super) fn continue_statement(
    codegen: &mut FunctionCodegen,
    statement: &Statement,
) -> Result<Flow, Diagnostic> {
    let Some((continue_block, _)) = codegen.loops.last().copied() else {
        return Err(Diagnostic::new(
            "`continue` outside of a loop",
            statement.location,
        ));
    };
    codegen.builder.ins().jump(continue_block, &[]);
    Ok(Flow::Terminated)
}

fn one_i64(codegen: &mut FunctionCodegen) -> Value {
    codegen.builder.ins().iconst(types::I64, 1)
}
