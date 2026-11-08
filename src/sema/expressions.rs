use super::{builtins, dates, types, Checker};
use crate::diagnostics::Diagnostic;
use crate::frontend::ast::{BinaryOp, Expr, Expression, UnaryOp};
use crate::sema::environment::MapLiteralKey;
use crate::sema::types::{BuiltinNamespace, SemaType};

impl Checker {
    pub(super) fn expression_type(&mut self, expr: &Expression) -> Result<SemaType, Diagnostic> {
        self.expression_with_expectation(expr, None)
    }

    pub(super) fn expression_with_expectation(
        &mut self,
        expr: &Expression,
        expected: Option<&SemaType>,
    ) -> Result<SemaType, Diagnostic> {
        match &expr.node {
            Expr::Int(_) => {
                if expected == Some(&SemaType::Float) {
                    Ok(SemaType::Float)
                } else {
                    Ok(SemaType::Int)
                }
            }
            Expr::Float(_) => Ok(SemaType::Float),
            Expr::Str(_) => Ok(SemaType::Str),
            Expr::Char(_) => Ok(SemaType::Char),
            Expr::Bool(_) => Ok(SemaType::Bool),
            Expr::RawJson(text) => {
                if let Err(error) = serde_json::from_str::<serde_json::Value>(text) {
                    return Err(Diagnostic::new(
                        format!("invalid json in the raw block: {error}"),
                        expr.location,
                    ));
                }
                Ok(SemaType::Json)
            }
            Expr::Ident(name) => self.ident_type(name, expr.location),
            Expr::Array(elements) => {
                let element_types = elements
                    .iter()
                    .map(|element| self.expression_type(element))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(SemaType::Array(element_types))
            }
            Expr::MapLiteral(entries) => self.map_literal_type(entries, expected, expr.location),
            Expr::Unary { op, operand } => {
                let operand_type = self.expression_type(operand)?;
                self.unary_type(*op, operand_type, expr.location)
            }
            Expr::Binary { op, left, right } => self.binary_type(*op, left, right, expr.location),
            Expr::FieldAccess { receiver, field } => {
                self.field_access_type(receiver, field, expr.location)
            }
            Expr::Index { receiver, index } => {
                let receiver_type = self.expression_type(receiver)?;
                let index_type = self.expression_type(index)?;
                match receiver_type {
                    SemaType::Table(table) => {
                        if index_type != SemaType::Int {
                            return Err(Diagnostic::new(
                                format!(
                                    "a table index must be int, got {}",
                                    types::describe(&index_type)
                                ),
                                expr.location,
                            ));
                        }
                        Ok(SemaType::Row(table.columns))
                    }
                    other => Err(Diagnostic::new(
                        format!("type {} cannot be indexed", types::describe(&other)),
                        expr.location,
                    )),
                }
            }
            Expr::MethodCall { receiver, method, arguments } => {
                builtins::method_call_type(self, receiver, method, arguments, expr.location)
            }
            Expr::Call { callee, arguments } => self.call_type(callee, arguments, expr.location),
        }
    }

    fn ident_type(&mut self, name: &str, location: crate::diagnostics::Location) -> Result<SemaType, Diagnostic> {
        if let Some(namespace) = BuiltinNamespace::from_name(name) {
            return Ok(SemaType::Namespace(namespace));
        }
        if let Some(entry) = self.environment.lookup(name) {
            return Ok(entry.ty.clone());
        }
        if let Some(value_entry) = self.environment.value(name) {
            return Ok(value_entry.ty.clone());
        }
        if let Some(table) = self.environment.table(name) {
            return Ok(SemaType::Table(table.clone()));
        }
        Err(Diagnostic::new(format!("undefined variable `{name}`"), location))
    }

    fn map_literal_type(
        &mut self,
        entries: &[crate::frontend::ast::MapEntry],
        expected: Option<&SemaType>,
        location: crate::diagnostics::Location,
    ) -> Result<SemaType, Diagnostic> {
        if entries.is_empty() {
            return match expected {
                Some(SemaType::Map { key, value }) => Ok(SemaType::Map {
                    key: key.clone(),
                    value: value.clone(),
                }),
                _ => Err(Diagnostic::new(
                    "cannot infer the key and value types of an empty map literal",
                    location,
                )),
            };
        }
        let key_type = self.expression_type(&entries[0].key)?;
        let value_type = self.expression_type(&entries[0].value)?;
        for entry in &entries[1..] {
            let entry_key = self.expression_type(&entry.key)?;
            let entry_value = self.expression_type(&entry.value)?;
            if entry_key != key_type {
                return Err(Diagnostic::new(
                    format!(
                        "map keys must share one type, got {} and {}",
                        types::describe(&key_type),
                        types::describe(&entry_key)
                    ),
                    location,
                ));
            }
            if entry_value != value_type {
                return Err(Diagnostic::new(
                    format!(
                        "map values must share one type, got {} and {}",
                        types::describe(&value_type),
                        types::describe(&entry_value)
                    ),
                    location,
                ));
            }
        }
        Ok(SemaType::Map {
            key: Box::new(key_type),
            value: Box::new(value_type),
        })
    }

    fn unary_type(
        &mut self,
        op: UnaryOp,
        operand_type: SemaType,
        location: crate::diagnostics::Location,
    ) -> Result<SemaType, Diagnostic> {
        match op {
            UnaryOp::Neg => match operand_type {
                SemaType::Int => Ok(SemaType::Int),
                SemaType::Float => Ok(SemaType::Float),
                other => Err(Diagnostic::new(
                    format!("cannot negate {}", types::describe(&other)),
                    location,
                )),
            },
            UnaryOp::Not => match operand_type {
                SemaType::Bool => Ok(SemaType::Bool),
                other => Err(Diagnostic::new(
                    format!("`NOT` requires a boolean, got {}", types::describe(&other)),
                    location,
                )),
            },
        }
    }

    fn binary_type(
        &mut self,
        op: BinaryOp,
        left: &Expression,
        right: &Expression,
        location: crate::diagnostics::Location,
    ) -> Result<SemaType, Diagnostic> {
        if op == BinaryOp::From {
            let source_type = self.expression_type(right)?;
            return match source_type {
                SemaType::Set(designator) => Ok(designator.element_type()),
                other => Err(Diagnostic::new(
                    format!(
                        "`random.choice from` requires a set, got {}",
                        types::describe(&other)
                    ),
                    location,
                )),
            };
        }
        let left_type = self.expression_type(left)?;
        let right_type = self.expression_type(right)?;
        types::binary_result(op, &left_type, &right_type)
            .map_err(|message| Diagnostic::new(message, location))
    }

    fn field_access_type(
        &mut self,
        receiver: &Expression,
        field: &str,
        location: crate::diagnostics::Location,
    ) -> Result<SemaType, Diagnostic> {
        let receiver_type = self.expression_type(receiver)?;
        match receiver_type {
            SemaType::Date => match field {
                "year" | "month" | "day" => Ok(SemaType::Int),
                _ => Err(Diagnostic::new(
                    format!("`date` has no field `{field}`, expected year, month or day"),
                    location,
                )),
            },
            SemaType::Json => Ok(SemaType::Json),
            SemaType::Row(columns) => columns
                .iter()
                .find(|column| column.name == field)
                .map(|column| column.ty.clone())
                .ok_or_else(|| {
                    Diagnostic::new(format!("unknown column `{field}` in the table row"), location)
                }),
            SemaType::Namespace(BuiltinNamespace::Random) if field == "choice" => {
                Ok(SemaType::RandomChoice)
            }
            other => Err(Diagnostic::new(
                format!("type {} has no field `{field}`", types::describe(&other)),
                location,
            )),
        }
    }

    fn call_type(
        &mut self,
        callee: &str,
        arguments: &[Expression],
        location: crate::diagnostics::Location,
    ) -> Result<SemaType, Diagnostic> {
        if let Some(signature) = self.environment.function(callee).cloned() {
            if arguments.len() != signature.parameters.len() {
                return Err(Diagnostic::new(
                    format!(
                        "function `{callee}` expects {} arguments, got {}",
                        signature.parameters.len(),
                        arguments.len()
                    ),
                    location,
                ));
            }
            for (position, (argument, parameter)) in
                arguments.iter().zip(&signature.parameters).enumerate()
            {
                let argument_type = self.expression_type(argument)?;
                if !types::assignable(parameter, &argument_type) {
                    return Err(Diagnostic::new(
                        format!(
                            "argument {} of `{callee}` must be {}, got {}",
                            position + 1,
                            types::describe(parameter),
                            types::describe(&argument_type)
                        ),
                        location,
                    ));
                }
            }
            return Ok(self.function_return_type(callee).unwrap_or(SemaType::Void));
        }
        if callee == "date" {
            return self.date_constructor(arguments, location);
        }
        Err(Diagnostic::new(format!("unknown function `{callee}`"), location))
    }

    fn date_constructor(
        &mut self,
        arguments: &[Expression],
        location: crate::diagnostics::Location,
    ) -> Result<SemaType, Diagnostic> {
        if arguments.is_empty() || arguments.len() > 2 {
            return Err(Diagnostic::new(
                format!(
                    "`date()` takes an ISO date and an optional format, got {} arguments",
                    arguments.len()
                ),
                location,
            ));
        }
        for argument in arguments {
            let argument_type = self.expression_type(argument)?;
            if argument_type != SemaType::Str {
                return Err(Diagnostic::new(
                    format!(
                        "`date()` arguments must be str, got {}",
                        types::describe(&argument_type)
                    ),
                    location,
                ));
            }
        }
        if let Some(text) = string_literal(&arguments[0]) {
            let format = arguments.get(1).and_then(string_literal);
            dates::validate_date_literal(text, format)
                .map_err(|message| Diagnostic::new(message, location))?;
        }
        Ok(SemaType::Date)
    }
}

pub(super) fn string_literal(expr: &Expression) -> Option<&str> {
    match &expr.node {
        Expr::Str(text) => Some(text),
        _ => None,
    }
}

pub(super) fn literal_map_keys(expr: &Expression) -> Option<Vec<MapLiteralKey>> {
    let Expr::MapLiteral(entries) = &expr.node else {
        return None;
    };
    let mut keys = Vec::new();
    for entry in entries {
        match &entry.key.node {
            Expr::Str(text) => keys.push(MapLiteralKey::Text(text.clone())),
            Expr::Int(value) => keys.push(MapLiteralKey::Integer(*value)),
            _ => return None,
        }
    }
    Some(keys)
}

pub(super) fn static_json_value(expr: &Expression) -> Option<serde_json::Value> {
    match &expr.node {
        Expr::RawJson(text) => serde_json::from_str(text).ok(),
        Expr::MethodCall { receiver, method, arguments } if method == "parse" => {
            let Expr::Ident(namespace) = &receiver.node else {
                return None;
            };
            if namespace != "json" {
                return None;
            }
            let Some(Expr::Str(text)) = arguments.first().map(|argument| &argument.node) else {
                return None;
            };
            serde_json::from_str(text).ok()
        }
        _ => None,
    }
}
