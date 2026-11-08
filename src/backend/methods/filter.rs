use std::collections::HashMap;

use super::super::functions;
use super::super::runtime;
use super::super::tables::{self, KIND_BOOL, KIND_DATE, KIND_FLOAT, KIND_INT, KIND_TEXT};
use super::super::values::ReturnShape;
use super::super::codegen;
use super::super::methods::tables::CONTEXT_SLOT;
use super::{Expectation, FunctionCodegen, Local, ShapeRef, Val};
use crate::diagnostics::Diagnostic;
use crate::frontend::ast::{Expr, Expression};
use cranelift::codegen::ir::condcodes::IntCC;
use cranelift::codegen::ir::{
    AbiParam, Function, InstBuilder, MemFlagsData, StackSlotData, StackSlotKind, UserFuncName,
    types,
};
use cranelift::frontend::FunctionBuilder;
use cranelift_module::{FuncId, Linkage, Module};

pub(in crate::backend) fn table_where(
    codegen: &mut FunctionCodegen,
    table: cranelift::codegen::ir::Value,
    shape: &ShapeRef,
    condition: &Expression,
    location: crate::diagnostics::Location,
) -> Result<Val, Diagnostic> {
    let mut captures: Vec<(String, Expectation)> = Vec::new();
    collect_captures(codegen, condition, shape, &mut captures, location)?;

    let name = tables::next_where_name();
    let mut signature = codegen.module.make_signature();
    signature.params.push(AbiParam::new(types::I64));
    signature.returns.push(AbiParam::new(types::I8));
    let id = codegen
        .module
        .declare_function(&name, Linkage::Export, &signature)
        .map_err(|error| Diagnostic::new(error.to_string(), location))?;
    build_where_fn(codegen, id, condition, shape, &captures, location)?;
    let reference = codegen
        .module
        .declare_func_in_func(id, &mut codegen.builder.func);

    let slot_count = shape.columns.len() + captures.len();
    let context_slot = codegen.builder.create_sized_stack_slot(StackSlotData::new(
        StackSlotKind::ExplicitSlot,
        (CONTEXT_SLOT * slot_count as i64) as u32,
        4,
    ));
    let context = codegen.builder.ins().stack_addr(types::I64, context_slot, 0);

    let capture_base = CONTEXT_SLOT * shape.columns.len() as i64;
    for (position, (capture_name, _capture_expectation)) in captures.iter().enumerate() {
        let offset = capture_base + CONTEXT_SLOT * position as i64;
        let offset_value = codegen.builder.ins().iconst(types::I64, offset);
        let address = codegen.builder.ins().iadd(context, offset_value);
        match codegen.locals.get(capture_name).cloned() {
            Some(Local::Scalar { ty, variable }) => {
                let value = codegen.builder.use_var(variable);
                let stored = match ty {
                    types::F64 => codegen
                        .builder
                        .ins()
                        .bitcast(types::I64, MemFlagsData::new(), value),
                    _ => value,
                };
                codegen
                    .builder
                    .ins()
                    .store(MemFlagsData::new(), stored, address, 0);
            }
            Some(Local::Days { variable }) => {
                let value = codegen.builder.use_var(variable);
                codegen
                    .builder
                    .ins()
                    .store(MemFlagsData::new(), value, address, 0);
            }
            Some(Local::Str { ptr, len }) => {
                let pointer_value = codegen.builder.use_var(ptr);
                let length_value = codegen.builder.use_var(len);
                codegen
                    .builder
                    .ins()
                    .store(MemFlagsData::new(), pointer_value, address, 0);
                codegen
                    .builder
                    .ins()
                    .store(MemFlagsData::new(), length_value, address, 8);
            }
            _ => {
                return Err(Diagnostic::new(
                    "only scalar and str variables can be used inside a where condition",
                    location,
                ));
            }
        }
    }

    let shape_bytes = tables::encode_shape(shape);
    let encoded = codegen.constant_bytes(&shape_bytes);
    let encoded_len = codegen
        .builder
        .ins()
        .iconst(types::I64, shape_bytes.len() as i64);
    let shape_ptr = codegen.call0(
        codegen.helpers.reference("xcx_table_shape_decode"),
        &[encoded, encoded_len],
    );
    let result = codegen.call0(
        codegen.helpers.reference("xcx_table_new_empty"),
        &[shape_ptr],
    );
    codegen.root_pointer(result);

    let index = codegen.builder.declare_var(types::I64);
    let zero_index = codegen.builder.ins().iconst(types::I64, 0);
    codegen.builder.def_var(index, zero_index);

    let header = codegen.builder.create_block();
    codegen.builder.ins().jump(header, &[]);
    codegen.builder.switch_to_block(header);

    let current = codegen.builder.use_var(index);
    let count = codegen.call0(codegen.helpers.reference("xcx_table_count"), &[table]);
    let more = codegen
        .builder
        .ins()
        .icmp(IntCC::SignedLessThan, current, count);

    let body = codegen.builder.create_block();
    let step = codegen.builder.create_block();
    let exit = codegen.builder.create_block();
    codegen.builder.ins().brif(more, body, &[], exit, &[]);
    codegen.builder.switch_to_block(body);
    codegen.builder.seal_block(body);

    codegen
        .builder
        .ins()
        .call(
            codegen.helpers.reference("xcx_table_fill_context"),
            &[table, current, context],
        );
    let matched = codegen.call0(reference, &[context]);
    let append_block = codegen.builder.create_block();
    codegen
        .builder
        .ins()
        .brif(matched, append_block, &[], step, &[]);
    codegen.builder.switch_to_block(append_block);
    codegen.builder.seal_block(append_block);
    codegen
        .builder
        .ins()
        .call(
            codegen.helpers.reference("xcx_table_append_row"),
            &[result, table, current],
        );
    codegen.builder.ins().jump(step, &[]);
    codegen.builder.switch_to_block(step);
    codegen.builder.seal_block(step);
    codegen.builder.ins().call(codegen.helpers.reference("xcx_collect"), &[]);
    let increment = codegen.builder.ins().iconst(types::I64, 1);
    let current = codegen.builder.use_var(index);
    let advanced = codegen.builder.ins().iadd(current, increment);
    codegen.builder.def_var(index, advanced);
    codegen.builder.ins().jump(header, &[]);
    codegen.builder.seal_block(header);

    codegen.builder.switch_to_block(exit);
    codegen.builder.seal_block(exit);
    Ok(Val::Table(result, Some(std::rc::Rc::clone(shape))))
}

#[allow(clippy::too_many_arguments)]
fn build_where_fn(
    outer: &mut FunctionCodegen,
    id: FuncId,
    condition: &Expression,
    shape: &ShapeRef,
    captures: &[(String, Expectation)],
    location: crate::diagnostics::Location,
) -> Result<(), Diagnostic> {
    let frontend_config = outer.module.isa().frontend_config();
    let mut signature = outer.module.make_signature();
    signature.params.push(AbiParam::new(types::I64));
    signature.returns.push(AbiParam::new(types::I8));
    let mut context = outer.module.make_context();
    context.func = Function::with_name_signature(
        UserFuncName::user(0, id.as_u32()),
        signature,
    );
    let mut builder_context = cranelift::frontend::FunctionBuilderContext::new();
    let mut locals = HashMap::new();
    {
        let mut builder = FunctionBuilder::new(&mut context.func, &mut builder_context);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);
        let helpers = runtime::Helpers::declare(&mut *outer.module, &mut builder);
        let references =
            functions::function_references(outer.functions, &mut *outer.module, &mut builder);
        let context_parameter = builder.block_params(entry)[0];
        let context_pointer = builder.declare_var(types::I64);
        builder.def_var(context_pointer, context_parameter);
        let context_value = builder.use_var(context_pointer);

        let mut offset = 0i64;
        for (column, kind, _) in &shape.columns {
            let offset_value = builder.ins().iconst(types::I64, offset);
            let address = builder.ins().iadd(context_value, offset_value);
            let local = match *kind {
                KIND_INT => {
                    let variable = builder.declare_var(types::I64);
                    let value = builder.ins().load(types::I64, MemFlagsData::new(), address, 0);
                    builder.def_var(variable, value);
                    Local::Scalar { ty: types::I64, variable }
                }
                KIND_FLOAT => {
                    let variable = builder.declare_var(types::F64);
                    let value = builder.ins().load(types::F64, MemFlagsData::new(), address, 0);
                    builder.def_var(variable, value);
                    Local::Scalar { ty: types::F64, variable }
                }
                KIND_TEXT => {
                    let ptr = builder.declare_var(types::I64);
                    let ptr_value = builder.ins().load(types::I64, MemFlagsData::new(), address, 0);
                    builder.def_var(ptr, ptr_value);
                    let len = builder.declare_var(types::I64);
                    let len_value = builder.ins().load(types::I64, MemFlagsData::new(), address, 8);
                    builder.def_var(len, len_value);
                    Local::Str { ptr, len }
                }
                KIND_BOOL => {
                    let variable = builder.declare_var(types::I8);
                    let raw = builder.ins().load(types::I8, MemFlagsData::new(), address, 0);
                    builder.def_var(variable, raw);
                    Local::Scalar { ty: types::I8, variable }
                }
                KIND_DATE => {
                    let variable = builder.declare_var(types::I64);
                    let value = builder.ins().load(types::I64, MemFlagsData::new(), address, 0);
                    builder.def_var(variable, value);
                    Local::Days { variable }
                }
                _ => {
                    let variable = builder.declare_var(types::I32);
                    let raw = builder.ins().load(types::I32, MemFlagsData::new(), address, 0);
                    builder.def_var(variable, raw);
                    Local::Scalar { ty: types::I32, variable }
                }
            };
            locals.insert(column.clone(), local);
            offset += CONTEXT_SLOT;
        }
        for (capture_name, capture_expectation) in captures {
            let offset_value = builder.ins().iconst(types::I64, offset);
            let address = builder.ins().iadd(context_value, offset_value);
            let local = match capture_expectation {
                Expectation::Int => {
                    let variable = builder.declare_var(types::I64);
                    let value = builder.ins().load(types::I64, MemFlagsData::new(), address, 0);
                    builder.def_var(variable, value);
                    Local::Scalar { ty: types::I64, variable }
                }
                Expectation::Float => {
                    let variable = builder.declare_var(types::F64);
                    let value = builder.ins().load(types::F64, MemFlagsData::new(), address, 0);
                    builder.def_var(variable, value);
                    Local::Scalar { ty: types::F64, variable }
                }
                Expectation::Bool => {
                    let variable = builder.declare_var(types::I8);
                    let raw = builder.ins().load(types::I8, MemFlagsData::new(), address, 0);
                    builder.def_var(variable, raw);
                    Local::Scalar { ty: types::I8, variable }
                }
                Expectation::Char => {
                    let variable = builder.declare_var(types::I32);
                    let raw = builder.ins().load(types::I32, MemFlagsData::new(), address, 0);
                    builder.def_var(variable, raw);
                    Local::Scalar { ty: types::I32, variable }
                }
                Expectation::Str => {
                    let ptr = builder.declare_var(types::I64);
                    let ptr_value = builder.ins().load(types::I64, MemFlagsData::new(), address, 0);
                    builder.def_var(ptr, ptr_value);
                    let len = builder.declare_var(types::I64);
                    let len_value = builder.ins().load(types::I64, MemFlagsData::new(), address, 8);
                    builder.def_var(len, len_value);
                    Local::Str { ptr, len }
                }
                Expectation::Date => {
                    let variable = builder.declare_var(types::I64);
                    let value = builder.ins().load(types::I64, MemFlagsData::new(), address, 0);
                    builder.def_var(variable, value);
                    Local::Days { variable }
                }
                Expectation::Any | Expectation::Json => {
                    return Err(Diagnostic::new(
                        "internal: unknown capture type in a where condition",
                        location,
                    ));
                }
            };
            locals.insert(capture_name.clone(), local);
            offset += CONTEXT_SLOT;
        }

        let mut cond = FunctionCodegen::new(
            builder,
            &mut *outer.module,
            helpers,
            outer.functions,
            &references,
            outer.global_tables,
            outer.global_opaques,
            outer.data_ids,
            None,
            ReturnShape::Scalar(types::I8),
            locals,
            false,
        );
        let result = cond.expression_expect(condition, Expectation::Bool)?;
        match result {
            Val::Bool(value) => {
                cond.leave_frame();
                cond.builder.ins().return_(&[value]);
            }
            other => {
                return Err(Diagnostic::new(
                    format!(
                        "internal: a where condition must be boolean, got {}",
                        codegen::val_description(&other)
                    ),
                    location,
                ));
            }
        }
        cond.builder.finalize(frontend_config);
    }
    outer
        .module
        .define_function(id, &mut context)
        .map_err(|error| Diagnostic::new(error.to_string(), location))?;
    outer.module.clear_context(&mut context);
    Ok(())
}

fn collect_captures(
    codegen: &mut FunctionCodegen,
    expression: &Expression,
    shape: &ShapeRef,
    captures: &mut Vec<(String, Expectation)>,
    location: crate::diagnostics::Location,
) -> Result<(), Diagnostic> {
    match &expression.node {
        Expr::Ident(name) => {
            if shape.columns.iter().any(|(column, _, _)| column == name) {
                return Ok(());
            }
            if captures.iter().any(|(existing, _)| existing == name) {
                return Ok(());
            }
            let expectation = match codegen.locals.get(name) {
                Some(Local::Scalar { ty, .. }) => match *ty {
                    types::F64 => Expectation::Float,
                    types::I8 => Expectation::Bool,
                    types::I32 => Expectation::Char,
                    _ => Expectation::Int,
                },
                Some(Local::Str { .. }) => Expectation::Str,
                Some(Local::Days { .. }) => Expectation::Date,
                _ => {
                    return Err(Diagnostic::new(
                        format!(
                            "`{name}` in a where condition must be a column of the table or a scalar variable"
                        ),
                        location,
                    ));
                }
            };
            captures.push((name.clone(), expectation));
            Ok(())
        }
        Expr::Int(_) | Expr::Float(_) | Expr::Str(_) | Expr::Bool(_) | Expr::Char(_) => Ok(()),
        Expr::Unary { operand, .. } => {
            collect_captures(codegen, operand, shape, captures, location)
        }
        Expr::Binary { left, right, .. } => {
            collect_captures(codegen, left, shape, captures, location)?;
            collect_captures(codegen, right, shape, captures, location)
        }
        Expr::Call { arguments, .. } => {
            for argument in arguments {
                collect_captures(codegen, argument, shape, captures, location)?;
            }
            Ok(())
        }
        Expr::MethodCall { receiver, arguments, .. } => {
            if !matches!(&receiver.node, Expr::Ident(name) if crate::sema::environment::RESERVED_NAMES.contains(&name.as_str())) {
                collect_captures(codegen, receiver, shape, captures, location)?;
            }
            for argument in arguments {
                collect_captures(codegen, argument, shape, captures, location)?;
            }
            Ok(())
        }
        Expr::FieldAccess { receiver, .. } => {
            collect_captures(codegen, receiver, shape, captures, location)
        }
        _ => Err(Diagnostic::new(
            "this expression is not supported inside a where condition yet",
            location,
        )),
    }
}
