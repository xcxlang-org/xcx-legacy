use std::cell::RefCell;
use std::collections::HashMap;

use super::{calls, composites, control_flow, methods, runtime, values};
use crate::diagnostics::Diagnostic;
use crate::frontend::ast::{BinaryOp, Expr, Statement, Stmt, UnaryOp};
use cranelift::codegen::ir::{
    FuncRef, InstBuilder, MemFlagsData, StackSlotData, StackSlotKind, Value, types,
};
use cranelift::frontend::{FunctionBuilder, Variable};
use cranelift::codegen::ir::Block as IrBlock;
use super::values::ReturnShape;
use cranelift_module::{DataId, Module};

mod operators;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Flow {
    FallThrough,
    Terminated,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Expectation {
    Any,
    Int,
    Float,
    Bool,
    Str,
    Char,
    Date,
    Json,
}

#[derive(Clone)]
pub(super) enum Local {
    Scalar { ty: types::Type, variable: Variable },
    Str { ptr: Variable, len: Variable },
    Opaque {
        kind: values::ObjectKind,
        variable: Variable,
        map_value: Option<char>,
    },
    Table { variable: Variable, shape: Option<ShapeRef> },
    Days { variable: Variable },
    JsonBuffer { variable: Variable },
}

pub(super) enum Val {
    Void,
    Int(Value),
    Float(Value),
    Bool(Value),
    Char(Value),
    Str { ptr: Value, len: Value },
    Set(Value),
    Map(Value, Option<char>),
    Table(Value, Option<ShapeRef>),
    Row { table: Value, index: Value, shape: ShapeRef },
    Keys(Value),
    Date(Value),
    Json(Value),
}

pub(super) type ShapeRef = std::rc::Rc<super::tables::TableShape>;

pub struct FunctionCodegen<'a> {
    pub(super) builder: FunctionBuilder<'a>,
    pub(super) module: &'a mut dyn Module,
    pub(super) helpers: runtime::Helpers,
    pub(super) functions: &'a super::functions::FunctionTable,
    pub(super) references: &'a HashMap<String, FuncRef>,
    pub(super) global_tables: &'a HashMap<String, ShapeRef>,
    pub(super) global_opaques: &'a HashMap<String, values::GlobalOpaque>,
    pub(super) data_ids: &'a RefCell<HashMap<Vec<u8>, DataId>>,
    pub(super) out_ptr: Option<Variable>,
    pub(super) return_shape: ReturnShape,
    pub(super) locals: HashMap<String, Local>,
    pub(super) loops: Vec<(IrBlock, IrBlock)>,
    pub(super) is_main: bool,
    next_root: i64,
}

impl<'a> FunctionCodegen<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        mut builder: FunctionBuilder<'a>,
        module: &'a mut dyn Module,
        helpers: runtime::Helpers,
        functions: &'a super::functions::FunctionTable,
        references: &'a HashMap<String, FuncRef>,
        global_tables: &'a HashMap<String, ShapeRef>,
        global_opaques: &'a HashMap<String, values::GlobalOpaque>,
        data_ids: &'a RefCell<HashMap<Vec<u8>, DataId>>,
        out_ptr: Option<Variable>,
        return_shape: ReturnShape,
        locals: HashMap<String, Local>,
        is_main: bool,
    ) -> Self {
        builder.ins().call(helpers.reference("xcx_frame_enter"), &[]);
        let mut codegen = Self {
            builder,
            module,
            helpers,
            functions,
            references,
            global_tables,
            global_opaques,
            data_ids,
            out_ptr,
            return_shape,
            locals,
            loops: Vec::new(),
            is_main,
            next_root: 0,
        };
        codegen.sync_roots();
        codegen
    }

    pub(super) fn constant_bytes(&mut self, contents: &[u8]) -> Value {
        let data_id = {
            let mut cache = self.data_ids.borrow_mut();
            match cache.get(contents) {
                Some(data_id) => *data_id,
                None => {
                    let data_id = self
                        .module
                        .declare_anonymous_data(false, false)
                        .expect("declaring constant data");
                    let mut description = cranelift_module::DataDescription::new();
                    description.define(contents.to_vec().into_boxed_slice());
                    self.module
                        .define_data(data_id, &description)
                        .expect("defining constant data");
                    cache.insert(contents.to_vec(), data_id);
                    data_id
                }
            }
        };
        let global_value = self
            .module
            .declare_data_in_func(data_id, &mut self.builder.func);
        self.builder
            .ins()
            .symbol_value(types::I64, global_value)
    }

    pub(super) fn constant_text(&mut self, text: &str) -> (Value, Value) {
        let contents: &[u8] = text.as_bytes();
        let ptr = self.constant_bytes(contents);
        let len = self
            .builder
            .ins()
            .iconst(types::I64, contents.len() as i64);
        (ptr, len)
    }

    pub(super) fn table_shape(&self, name: &str) -> Option<ShapeRef> {
        match self.locals.get(name) {
            Some(Local::Table { shape, .. }) => shape.clone(),
            _ => self.global_tables.get(name).cloned(),
        }
    }

    fn register_global_opaque(&mut self, name: &str, local: &Local) {
        if let Local::Opaque {
            kind: values::ObjectKind::Map | values::ObjectKind::Set,
            ..
        } = local
        {
            let Local::Opaque { variable, .. } = local else {
                unreachable!("the opaque local is matched above");
            };
            let current = self.builder.use_var(*variable);
            self.register_global_map(name, current);
        }
    }

    pub(super) fn register_global_map(&mut self, name: &str, value: Value) {
        let (name_ptr, name_len) = self.constant_text(name);
        self.builder.ins().call(
            self.helpers.reference("xcx_global_map_register"),
            &[name_ptr, name_len, value],
        );
    }

    pub(super) fn statement_flow(&mut self, statement: &Statement) -> Result<Flow, Diagnostic> {
        let flow = self.statement_inner(statement)?;
        if flow == Flow::FallThrough {
            self.sync_roots();
            self.builder.ins().call(self.helpers.reference("xcx_collect"), &[]);
        }
        Ok(flow)
    }

    fn statement_inner(&mut self, statement: &Statement) -> Result<Flow, Diagnostic> {
        match &statement.node {
            Stmt::Let { ty, bindings } => {
                let Some(storage) = values::storage(ty) else {
                    return Err(self.error(
                        statement,
                        format!(
                            "values of type `{}` are not generated yet",
                            self.type_description(ty)
                        ),
                    ));
                };
                let expectation = expectation_from_type(ty);
                let map_value = Self::map_value_letter(ty);
                for binding in bindings {
                    let local = self.declare_local(&storage, map_value);
                    if let Some(init) = &binding.init {
                        let value = self.expression_expect(init, expectation)?;
                        self.store(&local, value, &binding.name, statement)?;
                    }
                    if self.is_main {
                        self.register_global_opaque(&binding.name, &local);
                    }
                    self.locals.insert(binding.name.clone(), local);
                }
            }
            Stmt::LetSet { .. } => {
                return composites::let_set(self, statement);
            }
            Stmt::Assign { name, value } => {
                let local = self
                    .locals
                    .get(name)
                    .cloned()
                    .ok_or_else(|| self.error(statement, format!("undefined local `{name}`")))?;
                let value = self.expression_expect(value, local_expectation(&local))?;
                self.store(&local, value, name, statement)?;
            }
            Stmt::Print { value } => {
                let value = self.expression_expect(value, Expectation::Any)?;
                match &value {
                    Val::Int(v) => {
                        self.builder.ins().call(self.helpers.reference("xcx_print_int"), &[*v]);
                    }
                    Val::Float(v) => {
                        self.builder.ins().call(self.helpers.reference("xcx_print_float"), &[*v]);
                    }
                    Val::Bool(v) => {
                        self.builder.ins().call(self.helpers.reference("xcx_print_bool"), &[*v]);
                    }
                    Val::Char(v) => {
                        self.builder.ins().call(self.helpers.reference("xcx_print_char"), &[*v]);
                    }
                    Val::Str { ptr, len } => {
                        self.builder
                            .ins()
                            .call(self.helpers.reference("xcx_print_str"), &[*ptr, *len]);
                    }
                    Val::Set(ptr) => {
                        self.builder
                            .ins()
                            .call(self.helpers.reference("xcx_print_set"), &[*ptr]);
                    }
                    Val::Map(ptr, _) => {
                        self.builder
                            .ins()
                            .call(self.helpers.reference("xcx_map_show"), &[*ptr]);
                    }
                    Val::Table(ptr, _) => {
                        self.builder
                            .ins()
                            .call(self.helpers.reference("xcx_table_print"), &[*ptr]);
                    }
                    Val::Date(days) => {
                        self.builder
                            .ins()
                            .call(self.helpers.reference("xcx_print_date"), &[*days]);
                    }
                    Val::Json(ptr) => {
                        self.builder
                            .ins()
                            .call(self.helpers.reference("xcx_json_print"), &[*ptr]);
                    }
                    Val::Row { .. } | Val::Keys(_) | Val::Void => {
                        return Err(self.error(statement, "cannot print this value"));
                    }
                }
            }
            Stmt::Halt => {
                if self.is_main {
                    let zero = self.builder.ins().iconst(types::I64, 0);
                    self.leave_frame();
                    self.builder.ins().return_(&[zero]);
                } else {
                    self.builder.ins().call(self.helpers.reference("xcx_halt"), &[]);
                    self.builder.ins().trap(cranelift::codegen::ir::TrapCode::user(1).expect("nonzero trap code"));
                }
                return Ok(Flow::Terminated);
            }
            Stmt::ExpressionStatement { expr } => {
                if let Expr::Ident(name) = &expr.node
                    && self.references.contains_key(name)
                    && !self.locals.contains_key(name)
                {
                    calls::call_expression(self, name, &[], expr.location)?;
                    return Ok(Flow::FallThrough);
                }
                self.expression_expect(expr, Expectation::Any)?;
            }
            Stmt::Function { .. } | Stmt::Import { .. } => {}
            Stmt::Return { value } => {
                return calls::return_statement(self, value.as_ref(), statement);
            }
            Stmt::If { branches, otherwise } => {
                return control_flow::if_statement(self, branches, otherwise.as_ref());
            }
            Stmt::While { condition, body } => {
                return control_flow::while_statement(self, condition, body);
            }
            Stmt::ForRange { variable, start, end, body } => {
                return control_flow::for_range_statement(self, variable, start, end, body);
            }
            Stmt::ForIn { variable_type, variable, iterable, body } => {
                return control_flow::for_in_statement(
                    self,
                    variable_type,
                    variable,
                    iterable,
                    body,
                );
            }
            Stmt::Break => {
                return control_flow::break_statement(self, statement);
            }
            Stmt::Continue => {
                return control_flow::continue_statement(self, statement);
            }
            Stmt::Read { name } => {
                let local = match self.locals.get(name).cloned() {
                    Some(local) => local,
                    None => {
                        let local = Local::Str {
                            ptr: self.builder.declare_var(types::I64),
                            len: self.builder.declare_var(types::I64),
                        };
                        self.locals.insert(name.clone(), local.clone());
                        local
                    }
                };
                let value = read_local(self, &local, statement)?;
                self.store(&local, value, name, statement)?;
            }
            Stmt::Terminal { command } => match command.as_str() {
                "clean" => {
                    self.builder
                        .ins()
                        .call(self.helpers.reference("xcx_terminal_clean"), &[]);
                }
                "help" => {
                    self.builder
                        .ins()
                        .call(self.helpers.reference("xcx_terminal_help"), &[]);
                }
                "exit" => {
                    if self.is_main {
                        let zero = self.builder.ins().iconst(types::I64, 0);
                        self.leave_frame();
                        self.builder.ins().return_(&[zero]);
                        return Ok(Flow::Terminated);
                    }
                    self.builder.ins().call(self.helpers.reference("xcx_halt"), &[]);
                    self.builder.ins().trap(cranelift::codegen::ir::TrapCode::user(1).expect("nonzero trap code"));
                    return Ok(Flow::Terminated);
                }
                other => {
                    return Err(self.error(
                        statement,
                        format!("internal: unknown terminal command `{other}`"),
                    ));
                }
            },
            Stmt::Const { name, ty, init } => {
                let Some(storage) = values::storage(ty) else {
                    return Err(self.error(
                        statement,
                        format!(
                            "values of type `{}` are not generated yet",
                            self.type_description(ty)
                        ),
                    ));
                };
                let local = self.declare_local(&storage, Self::map_value_letter(ty));
                let expectation = self.expectation_from_type(ty);
                let value = self.expression_expect(init, expectation)?;
                self.store(&local, value, name, statement)?;
                if self.is_main {
                    self.register_global_opaque(name, &local);
                }
                self.locals.insert(name.clone(), local);
            }
            Stmt::LetTable { .. } => {
                return methods::let_table(self, statement);
            }
        }
        Ok(Flow::FallThrough)
    }

    pub(super) fn emit_body(&mut self, statements: &[Statement]) -> Result<Flow, Diagnostic> {
        let outer = self.locals.clone();
        let result = self.emit_statements(statements);
        self.locals = outer;
        result
    }

    fn emit_statements(&mut self, statements: &[Statement]) -> Result<Flow, Diagnostic> {
        for statement in statements {
            if self.statement_flow(statement)? == Flow::Terminated {
                return Ok(Flow::Terminated);
            }
        }
        Ok(Flow::FallThrough)
    }

    pub(super) fn expectation_from_type(&self, ty: &crate::frontend::ast::Type) -> Expectation {
        match ty {
            crate::frontend::ast::Type::Int => Expectation::Int,
            crate::frontend::ast::Type::Float => Expectation::Float,
            crate::frontend::ast::Type::Boolean => Expectation::Bool,
            crate::frontend::ast::Type::Str => Expectation::Str,
            crate::frontend::ast::Type::Date => Expectation::Date,
            crate::frontend::ast::Type::Json => Expectation::Json,
            _ => Expectation::Any,
        }
    }

    pub(super) fn map_value_letter(ty: &crate::frontend::ast::Type) -> Option<char> {
        let crate::frontend::ast::Type::Map { value, .. } = ty else {
            return None;
        };
        match **value {
            crate::frontend::ast::Type::Int => Some('i'),
            crate::frontend::ast::Type::Float => Some('f'),
            crate::frontend::ast::Type::Boolean => Some('b'),
            crate::frontend::ast::Type::Str => Some('s'),
            _ => None,
        }
    }

    fn declare_local(&mut self, storage: &values::Storage, map_value: Option<char>) -> Local {
        match storage {
            values::Storage::Scalar(ty) => Local::Scalar {
                ty: *ty,
                variable: self.builder.declare_var(*ty),
            },
            values::Storage::StrPair => Local::Str {
                ptr: self.builder.declare_var(types::I64),
                len: self.builder.declare_var(types::I64),
            },
            values::Storage::Opaque(kind) => match kind {
                values::ObjectKind::Table => Local::Table {
                    variable: self.builder.declare_var(types::I64),
                    shape: None,
                },
                other => Local::Opaque {
                    kind: *other,
                    variable: self.builder.declare_var(types::I64),
                    map_value,
                },
            },
            values::Storage::Days => Local::Days {
                variable: self.builder.declare_var(types::I64),
            },
            values::Storage::JsonBuffer => Local::JsonBuffer {
                variable: self.builder.declare_var(types::I64),
            },
        }
    }

    pub(super) fn store(
        &mut self,
        local: &Local,
        value: Val,
        name: &str,
        statement: &Statement,
    ) -> Result<(), Diagnostic> {
        match (local, value) {
            (Local::Scalar { ty, variable }, Val::Int(v)) if *ty == types::I64 => {
                self.builder.def_var(*variable, v);
            }
            (Local::Scalar { ty, variable }, Val::Float(v)) if *ty == types::F64 => {
                self.builder.def_var(*variable, v);
            }
            (Local::Scalar { ty, variable }, Val::Bool(v)) if *ty == types::I8 => {
                self.builder.def_var(*variable, v);
            }
            (Local::Scalar { ty, variable }, Val::Char(v)) if *ty == types::I32 => {
                self.builder.def_var(*variable, v);
            }
            (Local::Str { ptr, len }, Val::Str { ptr: p, len: l }) => {
                self.builder.def_var(*ptr, p);
                self.builder.def_var(*len, l);
            }
            (Local::Opaque { variable, .. }, Val::Set(v)) | (Local::Opaque { variable, .. }, Val::Map(v, _)) => {
                self.builder.def_var(*variable, v);
            }
            (Local::Table { variable, .. }, Val::Table(v, _)) => {
                self.builder.def_var(*variable, v);
            }
            (Local::Days { variable }, Val::Date(v)) => {
                self.builder.def_var(*variable, v);
            }
            (Local::JsonBuffer { variable }, Val::Json(v)) => {
                self.builder.def_var(*variable, v);
            }
            _ => {
                return Err(self.error(
                    statement,
                    format!("internal type mismatch while storing `{name}`"),
                ));
            }
        }
        Ok(())
    }

    pub(super) fn expression(&mut self, expr: &crate::frontend::ast::Expression) -> Result<Val, Diagnostic> {
        self.expression_expect(expr, Expectation::Any)
    }

    pub(super) fn expression_expect(
        &mut self,
        expr: &crate::frontend::ast::Expression,
        expectation: Expectation,
    ) -> Result<Val, Diagnostic> {
        let value = self.expression_inner(expr, expectation)?;
        self.root_temporary(&value);
        Ok(value)
    }

    fn expression_inner(
        &mut self,
        expr: &crate::frontend::ast::Expression,
        expectation: Expectation,
    ) -> Result<Val, Diagnostic> {
        match &expr.node {
            Expr::Int(value) => match expectation {
                Expectation::Float => Ok(Val::Float(self.builder.ins().f64const(*value as f64))),
                _ => Ok(Val::Int(self.builder.ins().iconst(types::I64, *value))),
            },
            Expr::Float(value) => Ok(Val::Float(self.builder.ins().f64const(*value))),
            Expr::Bool(value) => Ok(Val::Bool(self.builder.ins().iconst(types::I8, *value as i64))),
            Expr::Char(value) => Ok(Val::Char(
                self.builder
                    .ins()
                    .iconst(types::I32, *value as u32 as i64),
            )),
            Expr::Str(text) => self.string_literal(text),
            Expr::Ident(name) => match self.locals.get(name).cloned() {
                Some(Local::Scalar { ty, variable }) => {
                    let value = self.builder.use_var(variable);
                    Ok(match ty {
                        types::F64 => Val::Float(value),
                        types::I8 => Val::Bool(value),
                        types::I32 => Val::Char(value),
                        _ => Val::Int(value),
                    })
                }
                Some(Local::Str { ptr, len }) => Ok(Val::Str {
                    ptr: self.builder.use_var(ptr),
                    len: self.builder.use_var(len),
                }),
                Some(Local::Opaque { kind, variable, map_value }) => {
                    let value = self.builder.use_var(variable);
                    Ok(match kind {
                        values::ObjectKind::Set => Val::Set(value),
                        values::ObjectKind::Map => Val::Map(value, map_value),
                        values::ObjectKind::Table => {
                            unreachable!("tables are stored as Local::Table")
                        }
                    })
                }
                Some(Local::Table { variable, shape }) => Ok(Val::Table(
                    self.builder.use_var(variable),
                    shape.clone(),
                )),
                Some(Local::Days { variable }) => Ok(Val::Date(self.builder.use_var(variable))),
                Some(Local::JsonBuffer { variable }) => {
                    Ok(Val::Json(self.builder.use_var(variable)))
                }
                None => {
                    if let Some(shape) = self.global_tables.get(name).cloned() {
                        let (pointer, length) = self.constant_text(name);
                        let table = self.call0(
                            self.helpers.reference("xcx_global_table"),
                            &[pointer, length],
                        );
                        return Ok(Val::Table(table, Some(shape)));
                    }
                    if let Some(opaque) = self.global_opaques.get(name).copied() {
                        let (pointer, length) = self.constant_text(name);
                        let value = self.call0(
                            self.helpers.reference("xcx_global_map"),
                            &[pointer, length],
                        );
                        return Ok(match opaque.kind {
                            values::ObjectKind::Map => Val::Map(value, opaque.map_value),
                            values::ObjectKind::Set => Val::Set(value),
                            values::ObjectKind::Table => {
                                unreachable!("tables resolve through the shape map")
                            }
                        });
                    }
                    Err(Diagnostic::new(
                        format!("undefined local `{name}`"),
                        expr.location,
                    ))
                }
            },
            Expr::Unary { op, operand } => {
                let operand = self.expression_expect(operand, expectation)?;
                match op {
                    UnaryOp::Neg => match operand {
                        Val::Int(v) => Ok(Val::Int(self.builder.ins().ineg(v))),
                        Val::Float(v) => Ok(Val::Float(self.builder.ins().fneg(v))),
                        other => Err(self.expression_error(&other, "negation")),
                    },
                    UnaryOp::Not => match operand {
                        Val::Bool(v) => {
                            let one = self.builder.ins().iconst(types::I8, 1);
                            Ok(Val::Bool(self.builder.ins().bxor(v, one)))
                        }
                        other => Err(self.expression_error(&other, "`NOT`")),
                    },
                }
            }
            Expr::Binary { op, left, right } => {
                self.binary(*op, left, right, expr.location, expectation)
            }
            Expr::Call { callee, arguments } => {
                calls::call_expression(self, callee, arguments, expr.location)
            }
            Expr::MethodCall { receiver, method, arguments } => {
                methods::method_call(self, receiver, method, arguments, expectation, expr.location)
            }
            Expr::MapLiteral(entries) => composites::map_literal(self, entries, expr.location),
            Expr::RawJson(text) => {
                let (ptr, len) = self.constant_text(text);
                let json = self.call0(
                    self.helpers.reference("xcx_json_parse"),
                    &[ptr, len],
                );
                Ok(Val::Json(json))
            }
            Expr::Index { receiver, index } => {
                let receiver_value = self.expression_expect(receiver, Expectation::Any)?;
                let index_value = self.expression_expect(index, Expectation::Int)?;
                let (Val::Table(table, Some(shape)), Val::Int(row)) =
                    (&receiver_value, &index_value)
                else {
                    return Err(Diagnostic::new(
                        "indexing is only supported on tables declared with `let (table)`",
                        expr.location,
                    ));
                };
                Ok(Val::Row {
                    table: *table,
                    index: *row,
                    shape: std::rc::Rc::clone(shape),
                })
            }
            Expr::FieldAccess { receiver, field } => {
                let receiver_value = self.expression_expect(receiver, Expectation::Any)?;
                match &receiver_value {
                    Val::Row { table, index, shape } => {
                        self.row_field(table, *index, shape, field, expr.location)
                    }
                    Val::Date(days) => self.date_property(*days, field, expr.location),
                    Val::Json(json) => {
                        let (name_ptr, name_len) = self.constant_text(field);
                        let field_value = self.call0(
                            self.helpers.reference("xcx_json_field"),
                            &[*json, name_ptr, name_len],
                        );
                        Ok(Val::Json(field_value))
                    }
                    _ => Err(Diagnostic::new(
                        "field access is only supported on table rows, date properties and json fields",
                        expr.location,
                    )),
                }
            }
            Expr::Array(_) => Err(Diagnostic::new(
                "composite literals are generated in a later phase",
                expr.location,
            )),
        }
    }

    fn date_property(
        &mut self,
        days: Value,
        field: &str,
        location: crate::diagnostics::Location,
    ) -> Result<Val, Diagnostic> {
        let helper = match field {
            "year" => "xcx_date_year",
            "month" => "xcx_date_month",
            "day" => "xcx_date_day",
            _ => {
                return Err(Diagnostic::new(
                    format!("`date` has no field `{field}`, expected year, month or day"),
                    location,
                ));
            }
        };
        Ok(Val::Int(self.call0(self.helpers.reference(helper), &[days])))
    }

    pub(super) fn bool_expression(&mut self, expr: &crate::frontend::ast::Expression) -> Result<Value, Diagnostic> {
        match self.expression_expect(expr, Expectation::Bool)? {
            Val::Bool(value) => Ok(value),
            other => Err(self.expression_error(&other, "a condition")),
        }
    }

    pub(super) fn call0(&mut self, function: FuncRef, arguments: &[Value]) -> Value {
        let call = self.builder.ins().call(function, arguments);
        self.builder.inst_results(call)[0]
    }

    pub(super) fn leave_frame(&mut self) {
        self.builder.ins().call(self.helpers.reference("xcx_frame_leave"), &[]);
    }

    fn root_temporary(&mut self, value: &Val) {
        let pointer = match value {
            Val::Str { ptr, .. } | Val::Set(ptr) | Val::Map(ptr, _)
            | Val::Table(ptr, _) | Val::Keys(ptr) | Val::Json(ptr) => *ptr,
            Val::Row { table, .. } => *table,
            _ => return,
        };
        self.root_pointer(pointer);
    }

    pub(super) fn root_pointer(&mut self, pointer: Value) {
        let slot = self.next_root;
        self.next_root += 1;
        self.set_root(slot, pointer);
    }

    fn set_root(&mut self, slot: i64, pointer: Value) {
        let slot = self.builder.ins().iconst(types::I64, slot);
        self.builder.ins().call(self.helpers.reference("xcx_root_set"), &[slot, pointer]);
    }

    fn sync_roots(&mut self) {
        let variables: Vec<Variable> = self.locals.values().filter_map(|local| match local {
            Local::Str { ptr, .. } => Some(*ptr),
            Local::Opaque { variable, .. } | Local::Table { variable, .. }
            | Local::JsonBuffer { variable } => Some(*variable),
            _ => None,
        }).collect();
        for variable in variables {
            let pointer = self.builder.use_var(variable);
            self.set_root(-i64::from(variable.as_u32()) - 1, pointer);
        }
    }

    pub(super) fn emit_default_return(&mut self) -> Result<(), Diagnostic> {
        if self.is_main {
            let zero = self.builder.ins().iconst(types::I64, 0);
            self.leave_frame();
            self.builder.ins().return_(&[zero]);
        } else if self.return_shape == ReturnShape::Void {
            self.leave_frame();
            self.builder.ins().return_(&[]);
        } else {
            return Err(Diagnostic::new(
                "a value-returning function reached the end of its generated body",
                crate::diagnostics::Location { line: 0, column: 0 },
            ));
        }
        Ok(())
    }

    pub(super) fn string_literal(&mut self, text: &str) -> Result<Val, Diagnostic> {
        let (ptr, len) = self.constant_text(text);
        Ok(Val::Str { ptr, len })
    }

    fn row_field(
        &mut self,
        table: &Value,
        index: Value,
        shape: &ShapeRef,
        field: &str,
        location: crate::diagnostics::Location,
    ) -> Result<Val, Diagnostic> {
        let Some((column, kind)) = shape
            .columns
            .iter()
            .enumerate()
            .find_map(|(position, (name, kind, _))| {
                (name == field).then(|| (position, *kind))
            })
        else {
            return Err(Diagnostic::new(
                format!("unknown column `{field}` in table `{}`", shape.name),
                location,
            ));
        };
        let column_value = self.builder.ins().iconst(types::I64, column as i64);
        Ok(match kind {
            KIND_INT => Val::Int(self.call0(
                self.helpers.reference("xcx_table_get_int"),
                &[*table, index, column_value],
            )),
            KIND_FLOAT => Val::Float(self.call0(
                self.helpers.reference("xcx_table_get_float"),
                &[*table, index, column_value],
            )),
            KIND_TEXT => {
                let slot = self
                    .builder
                    .create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 8, 3));
                let address = self.builder.ins().stack_addr(types::I64, slot, 0);
                let ptr = self.call0(
                    self.helpers.reference("xcx_table_get_str"),
                    &[*table, index, column_value, address],
                );
                let len = self
                    .builder
                    .ins()
                    .load(types::I64, MemFlagsData::new(), address, 0);
                Val::Str { ptr, len }
            }
            KIND_BOOL => Val::Bool(self.call0(
                self.helpers.reference("xcx_table_get_bool"),
                &[*table, index, column_value],
            )),
            KIND_CHAR => Val::Char(self.call0(
                self.helpers.reference("xcx_table_get_char"),
                &[*table, index, column_value],
            )),
            KIND_DATE => Val::Date(self.call0(
                self.helpers.reference("xcx_table_get_date"),
                &[*table, index, column_value],
            )),
            other => {
                return Err(Diagnostic::new(
                    format!("column `{field}` has an unsupported kind {other}"),
                    location,
                ));
            }
        })
    }

    fn error(&self, statement: &Statement, message: impl Into<String>) -> Diagnostic {
        Diagnostic::new(message, statement.location)
    }

    pub(super) fn binary_error(
        &self,
        op: BinaryOp,
        left: &Val,
        right: &Val,
        location: crate::diagnostics::Location,
    ) -> Diagnostic {
        Diagnostic::new(
            format!(
                "internal type mismatch applying `{}` to {} and {}",
                op_spelling(op),
                val_description(left),
                val_description(right)
            ),
            location,
        )
    }

    pub(super) fn expression_error(&self, value: &Val, context: &str) -> Diagnostic {
        Diagnostic::new(
            format!(
                "internal type mismatch: {} requires a number, got {}",
                context,
                val_description(value)
            ),
            crate::diagnostics::Location { line: 0, column: 0 },
        )
    }

    fn type_description(&self, ty: &crate::frontend::ast::Type) -> String {
        match ty {
            crate::frontend::ast::Type::Int => "int".to_string(),
            crate::frontend::ast::Type::Float => "float".to_string(),
            crate::frontend::ast::Type::Str => "str".to_string(),
            crate::frontend::ast::Type::Boolean => "boolean".to_string(),
            crate::frontend::ast::Type::Date => "date".to_string(),
            crate::frontend::ast::Type::Json => "json".to_string(),
            crate::frontend::ast::Type::Set => "set".to_string(),
            crate::frontend::ast::Type::Table => "table".to_string(),
            crate::frontend::ast::Type::Map { .. } => "map".to_string(),
        }
    }
}

fn read_local(
    codegen: &mut FunctionCodegen,
    local: &Local,
    statement: &Statement,
) -> Result<Val, Diagnostic> {
    Ok(match local {
        Local::Scalar { ty, .. } => match *ty {
            types::F64 => Val::Float(codegen.call0(
                codegen.helpers.reference("xcx_read_float"),
                &[],
            )),
            types::I8 => Val::Bool(codegen.call0(
                codegen.helpers.reference("xcx_read_bool"),
                &[],
            )),
            _ => Val::Int(codegen.call0(codegen.helpers.reference("xcx_read_int"), &[])),
        },
        Local::Str { .. } => {
            let out_slot = codegen
                .builder
                .create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 8, 3));
            let out_address = codegen.builder.ins().stack_addr(types::I64, out_slot, 0);
            let ptr = codegen.call0(
                codegen.helpers.reference("xcx_read_str"),
                &[out_address],
            );
            let len = codegen
                .builder
                .ins()
                .load(types::I64, MemFlagsData::new(), out_address, 0);
            Val::Str { ptr, len }
        }
        _other => {
            return Err(Diagnostic::new(
                "internal: a read target must be int, float, boolean or str",
                statement.location,
            ));
        }
    })
}

fn local_expectation(local: &Local) -> Expectation {
    match local {
        Local::Scalar { ty, .. } => match *ty {
            types::F64 => Expectation::Float,
            types::I8 => Expectation::Bool,
            types::I32 => Expectation::Char,
            _ => Expectation::Int,
        },
        Local::Str { .. } => Expectation::Str,
        Local::Opaque { .. } | Local::Table { .. } => Expectation::Any,
        Local::Days { .. } => Expectation::Date,
        Local::JsonBuffer { .. } => Expectation::Json,
    }
}

fn expectation_from_type(ty: &crate::frontend::ast::Type) -> Expectation {
    match ty {
        crate::frontend::ast::Type::Int => Expectation::Int,
        crate::frontend::ast::Type::Float => Expectation::Float,
        crate::frontend::ast::Type::Boolean => Expectation::Bool,
        crate::frontend::ast::Type::Str => Expectation::Str,
        crate::frontend::ast::Type::Date => Expectation::Date,
        crate::frontend::ast::Type::Json => Expectation::Json,
        _ => Expectation::Any,
    }
}

fn key_letter_of(value: &Val) -> &'static str {
    match value {
        Val::Str { .. } => "s",
        _ => "i",
    }
}

use super::tables::{KIND_BOOL, KIND_CHAR, KIND_DATE, KIND_FLOAT, KIND_INT, KIND_TEXT};

fn op_spelling(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::Mod => "%",
        BinaryOp::Concat => "++",
        BinaryOp::Eq => "==",
        BinaryOp::NotEq => "!=",
        BinaryOp::Lt => "<",
        BinaryOp::Gt => ">",
        BinaryOp::Le => "<=",
        BinaryOp::Ge => ">=",
        BinaryOp::And => "AND",
        BinaryOp::Or => "OR",
        BinaryOp::Has => "has",
        BinaryOp::From => "from",
        BinaryOp::Union => "UNION",
        BinaryOp::Intersection => "INTERSECTION",
        BinaryOp::Difference => "DIFFERENCE",
        BinaryOp::SymmetricDifference => "SYMMETRIC_DIFFERENCE",
    }
}

pub(super) fn val_description(value: &Val) -> &'static str {
    match value {
        Val::Void => "void",
        Val::Int(_) => "int",
        Val::Float(_) => "float",
        Val::Bool(_) => "boolean",
        Val::Char(_) => "char",
        Val::Str { .. } => "str",
        Val::Set(_) => "set",
        Val::Map(..) => "map",
        Val::Keys(_) => "map keys",
        Val::Table(..) => "table",
        Val::Row { .. } => "table row",
        Val::Date(_) => "date",
        Val::Json(_) => "json",
    }
}
