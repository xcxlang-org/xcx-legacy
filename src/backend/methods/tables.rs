use super::super::tables::{self, KIND_BOOL, KIND_CHAR, KIND_DATE, KIND_FLOAT, KIND_INT, KIND_TEXT};
use super::super::codegen;
use super::{require_arity, zero, Expectation, FunctionCodegen, Local, ShapeRef, Val};
use crate::diagnostics::Diagnostic;
use crate::frontend::ast::{Expr, Expression, Statement, Stmt};
use cranelift::codegen::ir::{InstBuilder, MemFlagsData, StackSlotData, StackSlotKind, Value, types};

pub(super) const CONTEXT_SLOT: i64 = 16;

pub(in crate::backend) fn let_table(
    codegen: &mut FunctionCodegen,
    statement: &Statement,
) -> Result<codegen::Flow, Diagnostic> {
    let Stmt::LetTable { name, columns, init } = &statement.node else {
        unreachable!("let_table called with a non-table statement");
    };
    if let Some(expression) = init {
        let value = codegen.expression_expect(expression, Expectation::Any)?;
        let Val::Table(table, shape) = value else {
            return Err(Diagnostic::new(
                format!("internal: `{name}` must be initialized with a table"),
                statement.location,
            ));
        };
        let shape = shape.map(|shape| {
            std::rc::Rc::new(tables::TableShape {
                name: name.clone(),
                columns: shape.columns.clone(),
            })
        });
        let variable = codegen.builder.declare_var(types::I64);
        codegen.builder.def_var(variable, table);
        codegen.locals.insert(name.clone(), Local::Table { variable, shape });
        return Ok(codegen::Flow::FallThrough);
    }
    let mut shape_columns = Vec::new();
    for column in columns {
        shape_columns.push((column.name.clone(), kind_code(&column.ty, statement)?, column.auto));
    }
    let shape_ref = std::rc::Rc::new(tables::TableShape {
        name: name.clone(),
        columns: shape_columns,
    });
    let shape_bytes = tables::encode_shape(&shape_ref);
    let encoded = codegen.constant_bytes(&shape_bytes);
    let encoded_len = codegen
        .builder
        .ins()
        .iconst(types::I64, shape_bytes.len() as i64);
    let shape_ptr = codegen.call0(
        codegen.helpers.reference("xcx_table_shape_decode"),
        &[encoded, encoded_len],
    );
    let table = codegen.call0(
        codegen.helpers.reference("xcx_table_new_empty"),
        &[shape_ptr],
    );
    if codegen.is_main {
        let (name_ptr, name_len) = codegen.constant_text(name);
        codegen.builder.ins().call(
            codegen.helpers.reference("xcx_global_table_register"),
            &[name_ptr, name_len, table],
        );
    }
    let variable = codegen.builder.declare_var(types::I64);
    codegen.builder.def_var(variable, table);
    codegen.locals.insert(
        name.clone(),
        Local::Table { variable, shape: Some(std::rc::Rc::clone(&shape_ref)) },
    );
    Ok(codegen::Flow::FallThrough)
}

fn kind_code(ty: &crate::frontend::ast::Type, statement: &Statement) -> Result<i32, Diagnostic> {
    tables::column_kind(ty).ok_or_else(|| {
        Diagnostic::new(
            "table columns of this type are not generated yet (supported: int, float, str, boolean, date)",
            statement.location,
        )
    })
}

fn cell_parts(
    codegen: &mut FunctionCodegen,
    value: &Val,
    location: crate::diagnostics::Location,
) -> Result<[Value; 3], Diagnostic> {
    Ok(match value {
        Val::Int(v) => {
            let tag = codegen.builder.ins().iconst(types::I64, KIND_INT as i64);
            [tag, *v, zero(codegen)]
        }
        Val::Float(v) => {
            let tag = codegen.builder.ins().iconst(types::I64, KIND_FLOAT as i64);
            let bits = codegen.builder.ins().bitcast(types::I64, MemFlagsData::new(), *v);
            [tag, bits, zero(codegen)]
        }
        Val::Str { ptr, len } => {
            let tag = codegen.builder.ins().iconst(types::I64, KIND_TEXT as i64);
            [tag, *ptr, *len]
        }
        Val::Bool(v) => {
            let tag = codegen.builder.ins().iconst(types::I64, KIND_BOOL as i64);
            let widened = codegen.builder.ins().uextend(types::I64, *v);
            [tag, widened, zero(codegen)]
        }
        Val::Char(v) => {
            let tag = codegen.builder.ins().iconst(types::I64, KIND_CHAR as i64);
            let widened = codegen.builder.ins().uextend(types::I64, *v);
            [tag, widened, zero(codegen)]
        }
        Val::Date(v) => {
            let tag = codegen.builder.ins().iconst(types::I64, KIND_DATE as i64);
            [tag, *v, zero(codegen)]
        }
        _ => {
            return Err(Diagnostic::new(
                "table cells must be int, float, str, boolean or char",
                location,
            ));
        }
    })
}

pub(in crate::backend) fn table_insert(
    codegen: &mut FunctionCodegen,
    table: Value,
    shape: &ShapeRef,
    arguments: &[Expression],
    location: crate::diagnostics::Location,
) -> Result<Val, Diagnostic> {
    let non_auto = shape.columns.iter().filter(|(_, _, auto)| !auto).count();
    if arguments.len() != non_auto {
        return Err(Diagnostic::new(
            format!(
                "`insert` expects {non_auto} values (one per non-auto column), got {}",
                arguments.len()
            ),
            location,
        ));
    }
    let slot = codegen
        .builder
        .create_sized_stack_slot(StackSlotData::new(
            StackSlotKind::ExplicitSlot,
            24 * non_auto as u32,
            3,
        ));
    let base = codegen.builder.ins().stack_addr(types::I64, slot, 0);
    for (position, argument) in arguments.iter().enumerate() {
        let value = codegen.expression_expect(argument, Expectation::Any)?;
        let parts = cell_parts(codegen, &value, location)?;
        let offset = 24 * position as i64;
        for (part, delta) in parts.iter().zip([0i64, 8, 16]) {
            let delta_value = codegen
                .builder
                .ins()
                .iconst(types::I64, offset + delta);
            let address = codegen.builder.ins().iadd(base, delta_value);
            codegen
                .builder
                .ins()
                .store(MemFlagsData::new(), *part, address, 0);
        }
    }
    let count_value = codegen
        .builder
        .ins()
        .iconst(types::I64, non_auto as i64);
    codegen.builder.ins().call(
        codegen.helpers.reference("xcx_table_insert"),
        &[table, base, count_value],
    );
    Ok(Val::Void)
}

pub(in crate::backend) fn table_update(
    codegen: &mut FunctionCodegen,
    table: Value,
    arguments: &[Expression],
    location: crate::diagnostics::Location,
) -> Result<Val, Diagnostic> {
    if arguments.len() != 2 {
        return Err(Diagnostic::new(
            "internal: `update` expects a row index and an array of replacement values",
            location,
        ));
    }
    let Expr::Array(values) = &arguments[1].node else {
        return Err(Diagnostic::new(
            "internal: the second `update` argument must be an array of row values",
            location,
        ));
    };
    let count = values.len();
    let slot = codegen
        .builder
        .create_sized_stack_slot(StackSlotData::new(
            StackSlotKind::ExplicitSlot,
            24 * count as u32,
            3,
        ));
    let base = codegen.builder.ins().stack_addr(types::I64, slot, 0);
    for (position, argument) in values.iter().enumerate() {
        let value = codegen.expression_expect(argument, Expectation::Any)?;
        let parts = cell_parts(codegen, &value, location)?;
        let offset = 24 * position as i64;
        for (part, delta) in parts.iter().zip([0i64, 8, 16]) {
            let delta_value = codegen
                .builder
                .ins()
                .iconst(types::I64, offset + delta);
            let address = codegen.builder.ins().iadd(base, delta_value);
            codegen
                .builder
                .ins()
                .store(MemFlagsData::new(), *part, address, 0);
        }
    }
    let index = codegen.expression(&arguments[0])?;
    let Val::Int(index_value) = index else {
        return Err(Diagnostic::new(
            "internal: the first `update` argument (the row index) must be int",
            location,
        ));
    };
    let count_value = codegen.builder.ins().iconst(types::I64, count as i64);
    codegen.builder.ins().call(
        codegen.helpers.reference("xcx_table_update"),
        &[table, index_value, base, count_value],
    );
    Ok(Val::Void)
}

pub(in crate::backend) fn table_delete(
    codegen: &mut FunctionCodegen,
    table: Value,
    arguments: &[Expression],
    location: crate::diagnostics::Location,
) -> Result<Val, Diagnostic> {
    require_arity(arguments, 1, "delete", location)?;
    let index = codegen.expression(&arguments[0])?;
    let Val::Int(index_value) = index else {
        return Err(Diagnostic::new(
            "the `delete` row index must be int",
            location,
        ));
    };
    codegen
        .builder
        .ins()
        .call(codegen.helpers.reference("xcx_table_delete"), &[table, index_value]);
    Ok(Val::Void)
}

pub(in crate::backend) fn table_join(
    codegen: &mut FunctionCodegen,
    table: Value,
    shape: &Option<ShapeRef>,
    arguments: &[Expression],
    location: crate::diagnostics::Location,
) -> Result<Val, Diagnostic> {
    require_arity(arguments, 3, "join", location)?;
    let Expr::Ident(right_name) = &arguments[0].node else {
        return Err(Diagnostic::new(
            "lambda join syntax is not defined in the 1.2 documentation; join by key columns",
            location,
        ));
    };
    let Some(left_shape) = shape else {
        return Err(Diagnostic::new(
            "cannot determine the shape of the joined tables",
            location,
        ));
    };
    let Some(right_shape) = codegen.table_shape(right_name) else {
        return Err(Diagnostic::new(
            format!(
                "cannot determine the shape of `{right_name}` — only tables declared with `let (table)` have a known shape"
            ),
            location,
        ));
    };
    let Some(left_key) = string_literal(&arguments[1]) else {
        return Err(Diagnostic::new(
            "the left join key must be a column name literal",
            location,
        ));
    };
    let Some(right_key) = string_literal(&arguments[2]) else {
        return Err(Diagnostic::new(
            "the right join key must be a column name literal",
            location,
        ));
    };
    let left_key_index = column_index(left_shape, left_key).ok_or_else(|| {
        Diagnostic::new(
            format!("unknown join key `{left_key}` in the left table"),
            location,
        )
    })?;
    let right_key_index = column_index(&right_shape, right_key).ok_or_else(|| {
        Diagnostic::new(
            format!("unknown join key `{right_key}` in the right table"),
            location,
        )
    })?;

    let right_value = codegen.expression_expect(&arguments[0], Expectation::Any)?;
    let Val::Table(right_table, _) = right_value else {
        return Err(Diagnostic::new(
            "internal: the join argument must be a table",
            location,
        ));
    };

    let (left_name_ptr, left_name_len) = codegen.constant_text(&left_shape.name);
    let (right_name_ptr, right_name_len) =
        codegen.constant_text(&right_shape.name);
    let left_key_index_value = codegen
        .builder
        .ins()
        .iconst(types::I64, left_key_index as i64);
    let right_key_index_value = codegen
        .builder
        .ins()
        .iconst(types::I64, right_key_index as i64);
    let joined = codegen.call0(
        codegen.helpers.reference("xcx_table_join"),
        &[
            table,
            right_table,
            left_key_index_value,
            right_key_index_value,
            left_name_ptr,
            left_name_len,
            right_name_ptr,
            right_name_len,
        ],
    );

    let joined_shape = std::rc::Rc::new(tables::TableShape {
        name: format!("{}_{}", left_shape.name, right_shape.name),
        columns: tables::join_layout(
            &left_shape,
            &right_shape,
            left_key_index,
            right_key_index,
            &left_shape.name,
            &right_shape.name,
        )
        .columns
        .iter()
        .map(|(name, kind)| (name.clone(), *kind, false))
        .collect(),
    });
    Ok(Val::Table(joined, Some(joined_shape)))
}

fn string_literal(expression: &Expression) -> Option<&str> {
    match &expression.node {
        Expr::Str(text) => Some(text),
        _ => None,
    }
}

fn column_index(shape: &ShapeRef, name: &str) -> Option<usize> {
    shape.columns.iter().position(|(column, _, _)| column == name)
}
