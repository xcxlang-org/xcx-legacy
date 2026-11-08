pub mod calls;
pub mod codegen;
pub mod collections;
pub mod composites;
pub mod control_flow;
pub mod dates;
pub mod functions;
pub mod json;
pub mod maps;
pub(crate) mod memory;
pub mod methods;
pub mod runtime;
pub mod store;
pub mod tables;
pub mod values;

use crate::diagnostics::{Diagnostic, Location};
use crate::frontend::ast::Statement;
use codegen::Flow;
use cranelift::codegen::ir::{AbiParam, Function, UserFuncName, Value, types};
use cranelift::frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_module::{Linkage, Module, default_libcall_names};
use functions::FunctionEntry;
use std::collections::HashMap;
use values::{ReturnShape, SlotLayout};

#[allow(clippy::too_many_arguments)]
fn build_user_functions(
    module: &mut dyn Module,
    table: &functions::FunctionTable,
    global_tables: &HashMap<String, codegen::ShapeRef>,
    global_opaques: &HashMap<String, values::GlobalOpaque>,
    data_ids: &std::cell::RefCell<HashMap<Vec<u8>, cranelift_module::DataId>>,
    program: &[Statement],
    call_conv: cranelift::codegen::isa::CallConv,
) -> Result<(), Diagnostic> {
    for statement in program {
        if let crate::frontend::ast::Stmt::Function { name, parameters, body, .. } = &statement.node
        {
            let entry = table
                .get(name)
                .cloned()
                .expect("every function statement was collected");
            build_user_function(
                module,
                table,
                global_tables,
                global_opaques,
                data_ids,
                &entry,
                parameters,
                body,
                call_conv,
            )?;
        }
    }
    Ok(())
}

fn declare_program_main(module: &mut dyn Module) -> Result<cranelift_module::FuncId, Diagnostic> {
    let mut signature = module.make_signature();
    signature.returns.push(AbiParam::new(types::I64));
    module
        .declare_function("xcx_program_main", Linkage::Export, &signature)
        .map_err(|error| verification_error(error))
}

fn program_main_signature(module: &mut dyn Module) -> Result<cranelift::codegen::ir::Signature, Diagnostic> {
    let mut signature = module.make_signature();
    signature.returns.push(AbiParam::new(types::I64));
    Ok(signature)
}

fn build_program_main(
    module: &mut dyn Module,
    table: &functions::FunctionTable,
    global_tables: &HashMap<String, codegen::ShapeRef>,
    global_opaques: &HashMap<String, values::GlobalOpaque>,
    data_ids: &std::cell::RefCell<HashMap<Vec<u8>, cranelift_module::DataId>>,
    program: &[Statement],
    frontend_config: cranelift::codegen::isa::TargetFrontendConfig,
) -> Result<cranelift_module::FuncId, Diagnostic> {
    let main_id = declare_program_main(module)?;
    let mut context = module.make_context();
    context.func = Function::with_name_signature(
        UserFuncName::user(0, main_id.as_u32()),
        program_main_signature(module)?,
    );
    let mut builder_context = FunctionBuilderContext::new();
    {
        let mut builder = FunctionBuilder::new(&mut context.func, &mut builder_context);
        let entry_block = builder.create_block();
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);
        let helpers = runtime::Helpers::declare(module, &mut builder);
        let references = functions::function_references(table, module, &mut builder);
        let mut codegen = codegen::FunctionCodegen::new(
            builder,
            module,
            helpers,
            table,
            &references,
            global_tables,
            global_opaques,
            data_ids,
            None,
            ReturnShape::Scalar(types::I64),
            HashMap::new(),
            true,
        );
        let mut terminated = false;
        for statement in program {
            if codegen.statement_flow(statement)? == Flow::Terminated {
                terminated = true;
                break;
            }
        }
        if !terminated {
            codegen.emit_default_return()?;
        }
        codegen.builder.finalize(frontend_config);
    }
    module
        .define_function(main_id, &mut context)
        .map_err(|error| verification_error(error))?;
    module.clear_context(&mut context);
    Ok(main_id)
}

pub fn emit_object(
    program: &[Statement],
    inferred_returns: &HashMap<String, crate::sema::types::SemaType>,
    isa: std::sync::Arc<dyn cranelift::codegen::isa::TargetIsa>,
    object_path: &std::path::Path,
) -> Result<(), Diagnostic> {
    let mut object_builder = cranelift_object::ObjectBuilder::new(isa, "xcx", default_libcall_names())
        .map_err(|error| Diagnostic::new(error.to_string(), NO_LOCATION))?;
    object_builder.per_function_section(true).per_data_object_section(true);
    let mut module = cranelift_object::ObjectModule::new(object_builder);

    let table = functions::collect_and_declare(program, &mut module, inferred_returns)?;
    let global_tables = collect_table_shapes(program);
    let global_opaques = collect_global_opaques(program);
    let data_ids = std::cell::RefCell::new(HashMap::new());

    let call_conv = module.isa().default_call_conv();
    let frontend_config = module.isa().frontend_config();
    build_user_functions(
        &mut module,
        &table,
        &global_tables,
        &global_opaques,
        &data_ids,
        program,
        call_conv,
    )?;

    build_program_main(
        &mut module,
        &table,
        &global_tables,
        &global_opaques,
        &data_ids,
        program,
        frontend_config,
    )?;

    let product = module.finish();
    let bytes = product
        .emit()
        .map_err(|error| Diagnostic::new(error.to_string(), NO_LOCATION))?;
    std::fs::write(object_path, bytes).map_err(|error| {
        Diagnostic::new(
            format!("cannot write the object file `{}`: {error}", object_path.display()),
            NO_LOCATION,
        )
    })?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn build_user_function(
    module: &mut dyn Module,
    table: &functions::FunctionTable,
    global_tables: &HashMap<String, codegen::ShapeRef>,
    global_opaques: &HashMap<String, values::GlobalOpaque>,
    data_ids: &std::cell::RefCell<HashMap<Vec<u8>, cranelift_module::DataId>>,
    entry: &FunctionEntry,
    parameters: &[crate::frontend::ast::Parameter],
    body: &[Statement],
    call_conv: cranelift::codegen::isa::CallConv,
) -> Result<(), Diagnostic> {
    let frontend_config = module.isa().frontend_config();
    let mut context = module.make_context();
    context.func = Function::with_name_signature(
        UserFuncName::user(0, entry.id.as_u32()),
        functions::machine_signature(entry, call_conv),
    );
    let mut builder_context = FunctionBuilderContext::new();
    {
        let mut builder = FunctionBuilder::new(&mut context.func, &mut builder_context);
        let entry_block = builder.create_block();
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);
        let helpers = runtime::Helpers::declare(module, &mut builder);
        let references = functions::function_references(table, module, &mut builder);
        let block_parameters: Vec<Value> = builder.block_params(entry_block).to_vec();

        let mut locals = HashMap::new();
        let mut out_ptr = None;
        let mut position = 0usize;
        if entry.returns == ReturnShape::Str {
            let variable = builder.declare_var(types::I64);
            builder.def_var(variable, block_parameters[position]);
            position += 1;
            out_ptr = Some(variable);
        }
        for (parameter, slot) in parameters.iter().zip(&entry.parameters) {
            match slot {
                SlotLayout::Scalar(ty) => {
                    let variable = builder.declare_var(*ty);
                    builder.def_var(variable, block_parameters[position]);
                    position += 1;
                    locals.insert(
                        parameter.name.clone(),
                        codegen::Local::Scalar { ty: *ty, variable },
                    );
                }
                SlotLayout::Str => {
                    let ptr = builder.declare_var(types::I64);
                    builder.def_var(ptr, block_parameters[position]);
                    position += 1;
                    let len = builder.declare_var(types::I64);
                    builder.def_var(len, block_parameters[position]);
                    position += 1;
                    locals.insert(
                        parameter.name.clone(),
                        codegen::Local::Str { ptr, len },
                    );
                }
                SlotLayout::Opaque => {
                    let variable = builder.declare_var(types::I64);
                    builder.def_var(variable, block_parameters[position]);
                    position += 1;
                    locals.insert(
                        parameter.name.clone(),
                        codegen::Local::Table { variable, shape: None },
                    );
                }
                SlotLayout::Days => {
                    let variable = builder.declare_var(types::I64);
                    builder.def_var(variable, block_parameters[position]);
                    position += 1;
                    locals.insert(parameter.name.clone(), codegen::Local::Days { variable });
                }
            }
        }

        let mut codegen = codegen::FunctionCodegen::new(
            builder,
            module,
            helpers,
            table,
            &references,
            global_tables,
            global_opaques,
            data_ids,
            out_ptr,
            entry.returns.clone(),
            locals,
            false,
        );
        let mut terminated = false;
        for statement in body {
            if codegen.statement_flow(statement)? == Flow::Terminated {
                terminated = true;
                break;
            }
        }
        if !terminated {
            codegen.emit_default_return()?;
        }
        codegen.builder.finalize(frontend_config);
    }

    module
        .define_function(entry.id, &mut context)
        .map_err(|error| verification_error(error))?;
    module.clear_context(&mut context);
    Ok(())
}

fn verification_error(error: cranelift_module::ModuleError) -> Diagnostic {
    match error {
        cranelift_module::ModuleError::Compilation(codegen_error) => match codegen_error {
            cranelift::codegen::CodegenError::Verifier(errors) => {
                let details: Vec<String> = errors.0.iter().map(|e| e.to_string()).collect();
                Diagnostic::new(details.join("; "), NO_LOCATION)
            }
            other => Diagnostic::new(other.to_string(), NO_LOCATION),
        },
        other => Diagnostic::new(other.to_string(), NO_LOCATION),
    }
}

fn collect_global_opaques(statements: &[Statement]) -> HashMap<String, values::GlobalOpaque> {
    use crate::frontend::ast::Type;
    let mut opaques = HashMap::new();
    collect_global_opaques_from(statements, &mut |name, ty| {
        if matches!(
            ty,
            Type::Map { .. } | Type::Set
        ) {
            let kind = if ty == &Type::Set {
                values::ObjectKind::Set
            } else {
                values::ObjectKind::Map
            };
            opaques.insert(name.to_string(), values::GlobalOpaque {
                kind,
                map_value: codegen::FunctionCodegen::map_value_letter(ty),
            });
        }
    });
    opaques
}

fn collect_global_opaques_from(
    statements: &[Statement],
    visit: &mut dyn FnMut(&str, &crate::frontend::ast::Type),
) {
    use crate::frontend::ast::Stmt;
    for statement in statements {
        match &statement.node {
            Stmt::Let { ty, bindings } => {
                for binding in bindings {
                    visit(&binding.name, ty);
                }
            }
            Stmt::LetSet { name, .. } => visit(name, &crate::frontend::ast::Type::Set),
            Stmt::If { branches, otherwise } => {
                for branch in branches {
                    collect_global_opaques_from(&branch.body, visit);
                }
                if let Some(body) = otherwise {
                    collect_global_opaques_from(body, visit);
                }
            }
            Stmt::While { body, .. } => collect_global_opaques_from(body, visit),
            Stmt::ForRange { body, .. } | Stmt::ForIn { body, .. } => {
                collect_global_opaques_from(body, visit)
            }
            Stmt::Function { .. } => {}
            _ => {}
        }
    }
}

fn collect_table_shapes(statements: &[Statement]) -> HashMap<String, codegen::ShapeRef> {
    let mut tables = HashMap::new();
    collect_table_shapes_from(statements, &mut tables);
    tables
}

fn collect_table_shapes_from(
    statements: &[Statement],
    tables: &mut HashMap<String, codegen::ShapeRef>,
) {
    use crate::frontend::ast::Stmt;
    for statement in statements {
        match &statement.node {
            Stmt::LetTable { name, columns, init: None } => {
                let shape = codegen::ShapeRef::new(tables::TableShape {
                    name: name.clone(),
                    columns: columns
                        .iter()
                        .map(|column| {
                            (
                                column.name.clone(),
                                tables::column_kind(&column.ty)
                                    .expect("sema validated every declared column type"),
                                column.auto,
                            )
                        })
                        .collect(),
                });
                tables.insert(name.clone(), shape);
            }
            Stmt::If { branches, otherwise } => {
                for branch in branches {
                    collect_table_shapes_from(&branch.body, tables);
                }
                if let Some(body) = otherwise {
                    collect_table_shapes_from(body, tables);
                }
            }
            Stmt::While { body, .. } => collect_table_shapes_from(body, tables),
            Stmt::ForRange { body, .. } | Stmt::ForIn { body, .. } => {
                collect_table_shapes_from(body, tables)
            }
            Stmt::Function { .. } => {}
            _ => {}
        }
    }
}

const NO_LOCATION: Location = Location { line: 0, column: 0 };
