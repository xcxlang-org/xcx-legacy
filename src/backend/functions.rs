use std::collections::HashMap;

use crate::diagnostics::Diagnostic;
use crate::frontend::ast::{Statement, Stmt, Type};
use crate::sema::types::SemaType;
use cranelift::codegen::ir::{AbiParam, FuncRef, Signature, types};
use cranelift::frontend::FunctionBuilder;
use cranelift_module::{FuncId, Linkage, Module};

use super::values::{ReturnShape, SlotLayout, flatten_slots, return_shape, slot_layout};

#[derive(Clone, Debug)]
pub struct FunctionEntry {
    pub id: FuncId,
    pub parameters: Vec<SlotLayout>,
    pub returns: ReturnShape,
    pub table_shape: Option<super::codegen::ShapeRef>,
}

pub type FunctionTable = HashMap<String, FunctionEntry>;

pub fn collect_and_declare(
    program: &[Statement],
    module: &mut dyn Module,
    inferred_returns: &HashMap<String, SemaType>,
) -> Result<FunctionTable, Diagnostic> {
    let mut table = FunctionTable::new();
    let call_conv = module.isa().default_call_conv();

    for statement in program {
        let Stmt::Function { name, parameters, return_type, .. } = &statement.node else {
            continue;
        };
        let mut slots = Vec::new();
        for parameter in parameters {
            let Some(layout) = slot_layout(&parameter.ty) else {
                let described = crate::sema::types::describe(&crate::sema::types::sema_type(
                    &parameter.ty,
                ));
                return Err(Diagnostic::new(
                    format!("parameters of type `{described}` are not generated yet"),
                    statement.location,
                ));
            };
            slots.push(layout);
        }
        let Some(returns) = return_shape_for(return_type.as_ref(), name, inferred_returns) else {
            let described = match return_type.as_ref() {
                Some(ty) => crate::sema::types::describe(&crate::sema::types::sema_type(ty)),
                None => crate::sema::types::describe(
                    inferred_returns
                        .get(name)
                        .expect("an inferred return type is present here"),
                ),
            };
            return Err(Diagnostic::new(
                format!("return type `{described}` is not generated yet"),
                statement.location,
            ));
        };
        let table_shape = inferred_returns
            .get(name)
            .and_then(|sema_type| sema_table_shape(name, sema_type));

        let mut machine_parameters = flatten_slots(&slots);
        if returns == ReturnShape::Str {
            machine_parameters.insert(0, types::I64);
        }
        let signature = Signature {
            params: machine_parameters.iter().map(|&ty| AbiParam::new(ty)).collect(),
            returns: signature_returns(&returns),
            call_conv,
        };

        let id = module
            .declare_function(&format!("xcx_user_{name}"), Linkage::Export, &signature)
            .map_err(|error| Diagnostic::new(error.to_string(), statement.location))?;
        table.insert(
            name.clone(),
            FunctionEntry { id, parameters: slots, returns, table_shape },
        );
    }
    Ok(table)
}

fn return_shape_for(
    return_type: Option<&Type>,
    function: &str,
    inferred_returns: &HashMap<String, SemaType>,
) -> Option<ReturnShape> {
    match return_type {
        Some(ty) => return_shape(Some(ty)),
        None => match inferred_returns.get(function) {
            Some(sema_type) => sema_return_shape(sema_type),
            None => Some(ReturnShape::Void),
        },
    }
}

fn sema_return_shape(ty: &SemaType) -> Option<ReturnShape> {
    match ty {
        SemaType::Int => Some(ReturnShape::Scalar(types::I64)),
        SemaType::Float => Some(ReturnShape::Scalar(types::F64)),
        SemaType::Bool => Some(ReturnShape::Scalar(types::I8)),
        SemaType::Str => Some(ReturnShape::Str),
        SemaType::Table(_) => Some(ReturnShape::Opaque),
        _ => None,
    }
}

fn sema_table_shape(function: &str, ty: &SemaType) -> Option<super::codegen::ShapeRef> {
    let SemaType::Table(table) = ty else {
        return None;
    };
    let mut columns = Vec::with_capacity(table.columns.len());
    for column in &table.columns {
        let kind = super::tables::column_kind_from_sema(&column.ty)?;
        columns.push((column.name.clone(), kind, column.auto));
    }
    Some(super::codegen::ShapeRef::new(super::tables::TableShape {
        name: function.to_string(),
        columns,
    }))
}

fn signature_returns(returns: &ReturnShape) -> Vec<AbiParam> {
    match returns {
        ReturnShape::Scalar(ty) => vec![AbiParam::new(*ty)],
        ReturnShape::Opaque | ReturnShape::Days => vec![AbiParam::new(types::I64)],
        _ => Vec::new(),
    }
}

pub fn function_references(
    table: &FunctionTable,
    module: &mut dyn Module,
    builder: &mut FunctionBuilder,
) -> HashMap<String, FuncRef> {
    let mut references = HashMap::new();
    for (name, entry) in table {
        let reference = module.declare_func_in_func(entry.id, &mut builder.func);
        references.insert(name.clone(), reference);
    }
    references
}

pub fn machine_signature(
    entry: &FunctionEntry,
    call_conv: cranelift::codegen::isa::CallConv,
) -> Signature {
    let mut machine_parameters = flatten_slots(&entry.parameters);
    if entry.returns == ReturnShape::Str {
        machine_parameters.insert(0, types::I64);
    }
    let signature = Signature {
        params: machine_parameters.iter().map(|&ty| AbiParam::new(ty)).collect(),
        returns: signature_returns(&entry.returns),
        call_conv,
    };
    signature
}
