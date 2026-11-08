pub mod builtins;
pub mod dates;
pub mod environment;
pub(crate) mod flow;
pub mod expressions;
pub mod json;
pub mod statements;
pub mod types;

use crate::diagnostics::{Diagnostic, Location};
use crate::frontend::ast::{Parameter, Statement, Stmt, Type};
use environment::Environment;
use types::SemaType;
use std::collections::HashMap;

pub struct Analysis {
    pub function_returns: HashMap<String, SemaType>,
}

pub fn analyze(program: &[Statement]) -> Result<Analysis, Diagnostic> {
    let mut checker = Checker {
        environment: Environment::new(),
        function_return: None,
        loop_depth: 0,
        in_function: false,
        inferred_return: None,
        inferred_returns: HashMap::new(),
    };
    checker.environment.push_scope();
    checker.collect_functions(program)?;
    checker.check_program(program)?;
    checker.environment.pop_scope();
    Ok(Analysis {
        function_returns: checker.inferred_returns,
    })
}

pub fn check(program: &[Statement]) -> Result<(), Diagnostic> {
    analyze(program).map(|_| ())
}

pub struct Checker {
    environment: Environment,
    function_return: Option<Option<SemaType>>,
    loop_depth: usize,
    in_function: bool,
    inferred_return: Option<SemaType>,
    inferred_returns: HashMap<String, SemaType>,
}

impl Checker {
    fn collect_functions(&mut self, program: &[Statement]) -> Result<(), Diagnostic> {
        for statement in program {
            let Stmt::Function { name, parameters, return_type, .. } = &statement.node else {
                continue;
            };
            let parameter_types = parameters
                .iter()
                .map(|parameter| types::sema_type(&parameter.ty))
                .collect();
            let signature = environment::FunctionSignature {
                parameters: parameter_types,
                return_type: return_type.as_ref().map(types::sema_type),
            };
            self.environment
                .declare_function(name, signature)
                .map_err(|message| Diagnostic::new(message, statement.location))?;
        }
        Ok(())
    }

    fn check_program(&mut self, program: &[Statement]) -> Result<(), Diagnostic> {
        for statement in program {
            if let Stmt::Function { name, parameters, return_type, body, .. } = &statement.node {
                self.check_function_body(
                    name,
                    parameters,
                    return_type.as_ref(),
                    body,
                    statement.location,
                )?;
            } else {
                self.check_statement(statement)?;
            }
        }
        Ok(())
    }

    fn check_function_body(
        &mut self,
        name: &str,
        parameters: &[Parameter],
        return_type: Option<&Type>,
        body: &[Statement],
        location: Location,
    ) -> Result<(), Diagnostic> {
        let isolated = self.environment.isolated();
        let outer = std::mem::replace(&mut self.environment, isolated);
        self.environment.push_scope();
        for parameter in parameters {
            let entry = environment::VarEntry {
                ty: types::sema_type(&parameter.ty),
                mutable: true,
                map_literal_keys: None,
                json_literal: None,
            };
            self.environment
                .declare(&parameter.name, entry)
                .map_err(|message| Diagnostic::new(message, location))?;
        }
        let previous_return = self.function_return.take();
        self.function_return = Some(return_type.map(types::sema_type));
        let previous_depth = self.loop_depth;
        self.loop_depth = 0;
        let previous_body = std::mem::replace(&mut self.in_function, true);
        let previous_inferred = self.inferred_return.take();
        let mut failure = None;
        for statement in body {
            if let Err(error) = self.check_statement(statement) {
                failure = Some(error);
                break;
            }
        }
        if failure.is_none() && (return_type.is_some() || self.inferred_return.is_some()) {
            let flow = flow::analyze(body);
            if flow.fallthrough {
                failure = Some(Diagnostic::new(
                    format!("function `{name}` can finish without returning a value"),
                    location,
                ));
            } else if let Some(location) = flow.bare_return {
                failure = Some(Diagnostic::new(
                    format!("function `{name}` must return a value on every returning path"),
                    location,
                ));
            }
        }
        if failure.is_none() {
            if let Some(inferred) = self.inferred_return.take() {
                self.inferred_returns.insert(name.to_string(), inferred);
            }
        }
        self.inferred_return = previous_inferred;
        self.in_function = previous_body;
        self.loop_depth = previous_depth;
        self.function_return = previous_return;
        self.environment = outer;
        match failure {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    pub(crate) fn record_inferred_return(
        &mut self,
        actual: SemaType,
        location: Location,
    ) -> Result<(), Diagnostic> {
        match &self.inferred_return {
            Some(existing) => {
                let consistent = match (existing, &actual) {
                    (SemaType::Table(left), SemaType::Table(right)) => left.columns == right.columns,
                    _ => types::assignable(existing, &actual) && types::assignable(&actual, existing),
                };
                if consistent {
                    Ok(())
                } else {
                    Err(Diagnostic::new(
                        format!(
                            "inconsistent return types: {} and {}",
                            types::describe(existing),
                            types::describe(&actual)
                        ),
                        location,
                    ))
                }
            }
            None => {
                self.inferred_return = Some(actual);
                Ok(())
            }
        }
    }

    pub(crate) fn function_return_type(&self, name: &str) -> Option<SemaType> {
        self.inferred_returns.get(name).cloned().or_else(|| {
            self.environment.function(name).and_then(|signature| signature.return_type.clone())
        })
    }
}
