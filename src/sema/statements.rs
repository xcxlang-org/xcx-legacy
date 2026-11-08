use super::{expressions, types, Checker};
use crate::diagnostics::Diagnostic;
use crate::frontend::ast::{SetElement, Statement, Stmt};
use crate::sema::environment::VarEntry;
use crate::sema::types::{ColumnShape, SemaType, SetDesignator, TableType};

impl Checker {
    pub(super) fn check_statement(&mut self, statement: &Statement) -> Result<(), Diagnostic> {
        match &statement.node {
            Stmt::Let { ty, bindings } => {
                let expected = types::sema_type(ty);
                let global_opaque = !self.in_function
                    && matches!(
                        expected,
                        SemaType::Map { .. } | SemaType::Set(_) | SemaType::SetAny
                    );
                for binding in bindings {
                    let actual = match &binding.init {
                        Some(init) => {
                            let actual =
                                self.expression_with_expectation(init, Some(&expected))?;
                            if !types::assignable(&expected, &actual) {
                                return Err(Diagnostic::new(
                                    format!(
                                        "cannot initialize `{}` of type {} with {}",
                                        binding.name,
                                        types::describe(&expected),
                                        types::describe(&actual)
                                    ),
                                    statement.location,
                                ));
                            }
                            Some(actual)
                        }
                        None => None,
                    };
                    let map_literal_keys = binding
                        .init
                        .as_ref()
                        .and_then(expressions::literal_map_keys);
                    let json_literal = binding
                        .init
                        .as_ref()
                        .and_then(expressions::static_json_value);
                    let entry_type = actual.unwrap_or_else(|| expected.clone());
                    let entry = VarEntry {
                        ty: entry_type,
                        mutable: true,
                        map_literal_keys,
                        json_literal,
                    };
                    if global_opaque {
                        self.environment
                            .declare_value(&binding.name, entry)
                            .map_err(|message| Diagnostic::new(message, statement.location))?;
                    } else {
                        self.environment
                            .declare(&binding.name, entry)
                            .map_err(|message| Diagnostic::new(message, statement.location))?;
                    }
                }
            }
            Stmt::Const { name, ty, init } => {
                let expected = types::sema_type(ty);
                let actual = self.expression_with_expectation(init, Some(&expected))?;
                if !types::assignable(&expected, &actual) {
                    return Err(Diagnostic::new(
                        format!(
                            "cannot initialize `{name}` of type {} with {}",
                            types::describe(&expected),
                            types::describe(&actual)
                        ),
                        statement.location,
                    ));
                }
                let map_literal_keys = expressions::literal_map_keys(init);
                let json_literal = expressions::static_json_value(init);
                self.environment
                    .declare(
                        name,
                        VarEntry { ty: actual, mutable: false, map_literal_keys, json_literal },
                    )
                    .map_err(|message| Diagnostic::new(message, statement.location))?;
            }
            Stmt::LetSet { name, designator, elements } => {
                let Some(designator) = SetDesignator::from_spelling(designator) else {
                    unreachable!("the parser validated the set designator")
                };
                let element_type = designator.element_type();
                for element in elements {
                    self.check_set_element(&element_type, element, statement.location)?;
                }
                if self.in_function {
                    self.environment
                        .declare(
                            name,
                            VarEntry {
                                ty: SemaType::Set(designator),
                                mutable: true,
                                map_literal_keys: None,
                                json_literal: None,
                            },
                        )
                        .map_err(|message| Diagnostic::new(message, statement.location))?;
                } else {
                    self.environment
                        .declare_value(
                            name,
                            VarEntry {
                                ty: SemaType::Set(designator),
                                mutable: true,
                                map_literal_keys: None,
                                json_literal: None,
                            },
                        )
                        .map_err(|message| Diagnostic::new(message, statement.location))?;
                }
            }
            Stmt::LetTable { name, init: Some(expression), .. } => {
                let actual = self.expression_type(expression)?;
                let SemaType::Table(mut table) = actual else {
                    return Err(Diagnostic::new(
                        format!(
                            "cannot initialize `{name}` of type table with {}",
                            types::describe(&actual)
                        ),
                        statement.location,
                    ));
                };
                table.name = Some(name.clone());
                self.environment
                    .declare(
                        name,
                        VarEntry { ty: SemaType::Table(table), mutable: true, map_literal_keys: None, json_literal: None },
                    )
                    .map_err(|message| Diagnostic::new(message, statement.location))?;
            }
            Stmt::LetTable { name, columns, init: None } => {
                for column in columns {
                    if !matches!(
                        types::sema_type(&column.ty),
                        SemaType::Int | SemaType::Float | SemaType::Str | SemaType::Bool | SemaType::Date
                    ) {
                        return Err(Diagnostic::new(
                            "table columns of this type are not generated yet (supported: int, float, str, boolean, date)",
                            statement.location,
                        ));
                    }
                }
                let shape: Vec<ColumnShape> = columns
                    .iter()
                    .map(|column| ColumnShape {
                        name: column.name.clone(),
                        ty: types::sema_type(&column.ty),
                        auto: column.auto,
                    })
                    .collect();
                let table =
                    TableType { name: Some(name.clone()), columns: shape };
                if self.in_function {
                    self.environment
                        .declare(
                            name,
                            VarEntry { ty: SemaType::Table(table), mutable: true, map_literal_keys: None, json_literal: None },
                        )
                        .map_err(|message| Diagnostic::new(message, statement.location))?;
                } else {
                    self.environment
                        .declare_table(name, table)
                        .map_err(|message| Diagnostic::new(message, statement.location))?;
                }
            }
            Stmt::Assign { name, value } => {
                let entry_type = match self.environment.lookup(name) {
                    Some(entry) => {
                        if !entry.mutable {
                            return Err(Diagnostic::new(
                                format!("cannot modify constant `{name}`"),
                                statement.location,
                            ));
                        }
                        entry.ty.clone()
                    }
                None => {
                    if self.environment.table(name).is_some() {
                        return Err(Diagnostic::new(
                            format!("cannot assign to the table `{name}`"),
                            statement.location,
                        ));
                    }
                    if self.environment.value(name).is_some() {
                        return Err(Diagnostic::new(
                            format!("cannot assign to the value `{name}`"),
                            statement.location,
                        ));
                    }
                    return Err(Diagnostic::new(
                        format!("undefined variable `{name}`"),
                        statement.location,
                    ));
                }
            };
            let actual = self.expression_type(value)?;
            if !types::assignable(&entry_type, &actual) {
                return Err(Diagnostic::new(
                    format!(
                        "cannot assign {} to {} variable `{name}` — XCX performs no implicit type conversions",
                        types::describe(&actual),
                        types::describe(&entry_type)
                    ),
                    statement.location,
                ));
            }
            }
            Stmt::Print { value } => {
                let value_type = self.expression_type(value)?;
                if value_type == SemaType::Void {
                    return Err(Diagnostic::new(
                        "cannot print a value of type void",
                        statement.location,
                    ));
                }
            }
            Stmt::Read { name } => match self.environment.lookup(name) {
                Some(entry) if !entry.mutable => {
                    return Err(Diagnostic::new(
                        format!("cannot modify constant `{name}`"),
                        statement.location,
                    ));
                }
                Some(entry) => {
                    if !matches!(
                        entry.ty,
                        SemaType::Int | SemaType::Float | SemaType::Bool | SemaType::Str
                    ) {
                        return Err(Diagnostic::new(
                            format!(
                                "cannot read into a variable of type {}",
                                types::describe(&entry.ty)
                            ),
                            statement.location,
                        ));
                    }
                }
                None => {
                    self.environment
                        .declare(
                            name,
                            VarEntry {
                                ty: SemaType::Str,
                                mutable: true,
                                map_literal_keys: None,
                                json_literal: None,
                            },
                        )
                        .map_err(|message| Diagnostic::new(message, statement.location))?;
                }
            },
            Stmt::Halt => {}
            Stmt::Break => {
                if self.loop_depth == 0 {
                    return Err(Diagnostic::new("`break` outside of a loop", statement.location));
                }
            }
            Stmt::Continue => {
                if self.loop_depth == 0 {
                    return Err(Diagnostic::new(
                        "`continue` outside of a loop",
                        statement.location,
                    ));
                }
            }
            Stmt::Return { value } => self.check_return(value.as_ref(), statement.location)?,
            Stmt::If { branches, otherwise } => {
                for branch in branches {
                    let condition_type = self.expression_type(&branch.condition)?;
                    if condition_type != SemaType::Bool {
                        return Err(Diagnostic::new(
                            format!(
                                "the `if` condition must be boolean, got {}",
                                types::describe(&condition_type)
                            ),
                            statement.location,
                        ));
                    }
                    self.check_scoped_body(&branch.body)?;
                }
                if let Some(body) = otherwise {
                    self.check_scoped_body(body)?;
                }
            }
            Stmt::While { condition, body } => {
                let condition_type = self.expression_type(condition)?;
                if condition_type != SemaType::Bool {
                    return Err(Diagnostic::new(
                        format!(
                            "the `while` condition must be boolean, got {}",
                            types::describe(&condition_type)
                        ),
                        statement.location,
                    ));
                }
                self.check_looped_body(body)?;
            }
            Stmt::ForRange { variable, start, end, body } => {
                for bound in [start, end] {
                    let bound_type = self.expression_type(bound)?;
                    if bound_type != SemaType::Int {
                        return Err(Diagnostic::new(
                            format!(
                                "the `for` range bounds must be int, got {}",
                                types::describe(&bound_type)
                            ),
                            statement.location,
                        ));
                    }
                }
                self.environment.push_scope();
                self.environment
                    .declare(
                        variable,
                        VarEntry { ty: SemaType::Int, mutable: true, map_literal_keys: None, json_literal: None },
                    )
                    .map_err(|message| Diagnostic::new(message, statement.location))?;
                self.check_looped_body(body)?;
                self.environment.pop_scope();
            }
            Stmt::ForIn { variable_type, variable, iterable, body } => {
                let iterable_type = self.expression_type(iterable)?;
                let SemaType::Keys(key_type) = iterable_type else {
                    return Err(Diagnostic::new(
                        format!(
                            "the `for … in` loop requires map keys(), got {}",
                            types::describe(&iterable_type)
                        ),
                        statement.location,
                    ));
                };
                let declared = types::sema_type(variable_type);
                if !types::assignable(&declared, &key_type) {
                    return Err(Diagnostic::new(
                        format!(
                            "the loop variable is declared as {}, but the map keys are {}",
                            types::describe(&declared),
                            types::describe(&key_type)
                        ),
                        statement.location,
                    ));
                }
                self.environment.push_scope();
                self.environment
                    .declare(
                        variable,
                        VarEntry { ty: *key_type, mutable: true, map_literal_keys: None, json_literal: None },
                    )
                    .map_err(|message| Diagnostic::new(message, statement.location))?;
                self.check_looped_body(body)?;
                self.environment.pop_scope();
            }
            Stmt::Function { .. } => {
                return Err(Diagnostic::new(
                    "functions can only be declared at the top level",
                    statement.location,
                ));
            }
            Stmt::Import { .. } => {}
            Stmt::Terminal { command } => {
                if !matches!(command.as_str(), "clean" | "exit" | "help") {
                    return Err(Diagnostic::new(
                        format!(
                            "unknown terminal command `{command}`, expected clean, exit or help"
                        ),
                        statement.location,
                    ));
                }
            }
            Stmt::ExpressionStatement { expr } => {
                if let crate::frontend::ast::Expr::Ident(name) = &expr.node {
                    if self.environment.function(name).is_some() {
                        return Ok(());
                    }
                }
                self.expression_type(expr)?;
            }
        }
        Ok(())
    }

    fn check_return(
        &mut self,
        value: Option<&crate::frontend::ast::Expression>,
        location: crate::diagnostics::Location,
    ) -> Result<(), Diagnostic> {
        let expected = self.function_return.clone();
        let Some(expected) = expected else {
            return Err(Diagnostic::new("return outside of a function", location));
        };
        match (expected, value) {
            (None, Some(value)) => {
                let actual = self.expression_type(value)?;
                self.record_inferred_return(actual, location)
            }
            (None, None) => Ok(()),
            (Some(expected), None) => Err(Diagnostic::new(
                format!(
                    "this function must return a value of type {}",
                    types::describe(&expected)
                ),
                location,
            )),
            (Some(expected), Some(value)) => {
                let actual = self.expression_type(value)?;
                if !types::assignable(&expected, &actual) {
                    return Err(Diagnostic::new(
                        format!(
                            "cannot return {} from a function returning {}",
                            types::describe(&actual),
                            types::describe(&expected)
                        ),
                        location,
                    ));
                }
                if matches!(expected, SemaType::Table(_)) {
                    self.record_inferred_return(actual, location)?;
                }
                Ok(())
            }
        }
    }

    fn check_set_element(
        &mut self,
        element_type: &SemaType,
        element: &SetElement,
        location: crate::diagnostics::Location,
    ) -> Result<(), Diagnostic> {
        match element {
            SetElement::Value(value) => {
                let value_type = self.expression_type(value)?;
                if !types::assignable(element_type, &value_type) {
                    return Err(Diagnostic::new(
                        format!(
                            "the set requires elements of type {}, got {}",
                            types::describe(element_type),
                            types::describe(&value_type)
                        ),
                        location,
                    ));
                }
                Ok(())
            }
            SetElement::Range { start, end, step } => {
                if !matches!(
                    element_type,
                    SemaType::Int | SemaType::Float
                ) {
                    return Err(Diagnostic::new(
                        "only numeric sets (N, Z, Q) support ranges",
                        location,
                    ));
                }
                let mut parts: Vec<&crate::frontend::ast::Expression> = vec![start, end];
                if let Some(step) = step {
                    parts.push(step);
                }
                for part in parts {
                    let part_type = self.expression_type(part)?;
                    if !types::assignable(element_type, &part_type) {
                        return Err(Diagnostic::new(
                            format!(
                                "the range bounds must be {}, got {}",
                                types::describe(element_type),
                                types::describe(&part_type)
                            ),
                            location,
                        ));
                    }
                }
                Ok(())
            }
        }
    }

    fn check_scoped_body(&mut self, body: &[Statement]) -> Result<(), Diagnostic> {
        self.environment.push_scope();
        for statement in body {
            self.check_statement(statement)?;
        }
        self.environment.pop_scope();
        Ok(())
    }

    fn check_looped_body(&mut self, body: &[Statement]) -> Result<(), Diagnostic> {
        self.loop_depth += 1;
        let result = self.check_scoped_body(body);
        self.loop_depth -= 1;
        result
    }
}
