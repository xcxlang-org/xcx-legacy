use super::Parser;
use crate::diagnostics::Diagnostic;
use crate::frontend::ast::{Binding, Column, Parameter, SetElement, Stmt, Type};
use crate::frontend::token::TokenKind;

impl Parser {
    pub(super) fn let_statement(&mut self) -> Result<Stmt, Diagnostic> {
        self.bump();
        self.expect(&TokenKind::LParen, "`(` after `let`")?;
        let ty = self.type_()?;
        self.expect(&TokenKind::RParen, "`)` after the type")?;
        let first_name = self.expect_ident("a variable name")?;

        if self.at(&TokenKind::LBrace) {
            return match ty {
                Type::Set => self.set_body(first_name),
                Type::Table => self.table_body(first_name),
                _ => Err(self.error_here("only `set` and `table` declarations take a `{}` body")),
            };
        }

        if ty == Type::Table && self.at(&TokenKind::Assign) {
            self.bump();
            let init = self.expression()?;
            self.expect_semi("the table declaration")?;
            return Ok(Stmt::LetTable {
                name: first_name,
                columns: Vec::new(),
                init: Some(init),
            });
        }

        let mut bindings = Vec::new();
        let mut requires_initializers = false;
        loop {
            let name = if bindings.is_empty() {
                first_name.clone()
            } else {
                self.expect_ident("a variable name after `,`")?
            };
            let init = if self.at(&TokenKind::Assign) || self.at(&TokenKind::RawBlockOpen) {
                if self.at(&TokenKind::Assign) {
                    self.bump();
                }
                Some(self.expression()?)
            } else {
                None
            };
            if init.is_some() {
                requires_initializers = true;
            } else if requires_initializers {
                return Err(self.error_here(
                    "every binding in a multiple declaration must be initialized",
                ));
            }
            bindings.push(Binding { name, init });
            if self.at(&TokenKind::Comma) {
                self.bump();
            } else {
                break;
            }
        }
        self.expect_semi("the declaration")?;
        Ok(Stmt::Let { ty, bindings })
    }

    pub(super) fn const_statement(&mut self) -> Result<Stmt, Diagnostic> {
        self.bump();
        self.expect(&TokenKind::LParen, "`(` after `const`")?;
        let ty = self.type_()?;
        self.expect(&TokenKind::RParen, "`)` after the type")?;
        let name = self.expect_ident("a constant name")?;
        if !self.at(&TokenKind::Assign) {
            return Err(self.error_here("constants require an initializer"));
        }
        self.bump();
        let init = self.expression()?;
        self.expect_semi("the constant initializer")?;
        Ok(Stmt::Const { name, ty, init })
    }

    pub(super) fn set_body(&mut self, name: String) -> Result<Stmt, Diagnostic> {
        self.expect(&TokenKind::LBrace, "`{`")?;
        let designator = self.expect_ident("a set designator (N, Z, Q, S, B or C)")?;
        if !matches!(designator.as_str(), "N" | "Z" | "Q" | "S" | "B" | "C") {
            return Err(self.error_here(format!(
                "unknown set designator `{designator}`, expected N, Z, Q, S, B or C"
            )));
        }
        self.expect(&TokenKind::Assign, "`=` after the set designator")?;
        let mut elements = Vec::new();
        loop {
            let start = self.expression()?;
            if self.at(&TokenKind::Comma) && self.peek_at(1) == Some(&TokenKind::Comma) {
                self.bump();
                self.bump();
                let end = self.expression()?;
                let step =
                    if self.at(&TokenKind::Comma) && self.peek_at(1) == Some(&TokenKind::Comma) {
                        self.bump();
                        self.bump();
                        Some(self.expression()?)
                    } else {
                        None
                    };
                elements.push(SetElement::Range { start, end, step });
            } else {
                elements.push(SetElement::Value(start));
            }
            if self.at(&TokenKind::Comma) {
                self.bump();
            } else {
                break;
            }
        }
        self.expect(&TokenKind::RBrace, "`}` to close the set body")?;
        self.expect_semi("the set declaration")?;
        Ok(Stmt::LetSet { name, designator, elements })
    }

    pub(super) fn table_body(&mut self, name: String) -> Result<Stmt, Diagnostic> {
        self.expect(&TokenKind::LBrace, "`{`")?;
        let section = self.expect_ident("`columns`")?;
        if section != "columns" {
            return Err(self.error_here("a table body starts with `columns = [...]`"));
        }
        self.expect(&TokenKind::Assign, "`=` after `columns`")?;
        self.expect(&TokenKind::LBracket, "`[` opening the column list")?;
        let mut columns = Vec::new();
        loop {
            let column_name = self.expect_ident("a column name")?;
            self.expect(&TokenKind::ColonColon, "`::` after the column name")?;
            let ty = self.type_()?;
            let auto = if self.at(&TokenKind::At) {
                self.bump();
                let marker = self.expect_ident("`auto` after `@`")?;
                if marker != "auto" {
                    return Err(self.error_here(format!(
                        "unknown column marker `@{marker}`, expected `@auto`"
                    )));
                }
                true
            } else {
                false
            };
            columns.push(Column { name: column_name, ty, auto });
            if self.at(&TokenKind::Comma) {
                self.bump();
            } else {
                break;
            }
        }
        self.expect(&TokenKind::RBracket, "`]` closing the column list")?;
        let rows_section = self.expect_ident("`rows`")?;
        if rows_section != "rows" {
            return Err(self.error_here("a table body continues with `rows = [...]`"));
        }
        self.expect(&TokenKind::Assign, "`=` after `rows`")?;
        self.expect(&TokenKind::LBracket, "`[` opening the rows list")?;
        while !self.at(&TokenKind::RBracket) {
            self.expression()?;
            if self.at(&TokenKind::Comma) {
                self.bump();
            } else {
                break;
            }
        }
        self.expect(&TokenKind::RBracket, "`]` closing the rows list")?;
        self.expect(&TokenKind::RBrace, "`}` to close the table body")?;
        self.expect_semi("the table declaration")?;
        Ok(Stmt::LetTable { name, columns, init: None })
    }

    pub(super) fn function_statement(&mut self) -> Result<Stmt, Diagnostic> {
        self.bump();
        self.expect(&TokenKind::Func, "`func` after `dec`")?;
        self.expect(&TokenKind::LParen, "`(` after `func`")?;
        let name = self.expect_ident("a function name")?;
        let mut parameters = Vec::new();
        let mut return_type = None;
        if self.at(&TokenKind::ColonColon) {
            self.bump();
            loop {
                self.expect(&TokenKind::Let, "`let` in a parameter declaration")?;
                self.expect(&TokenKind::LParen, "`(` after `let`")?;
                let ty = self.type_()?;
                self.expect(&TokenKind::RParen, "`)` after the parameter type")?;
                let parameter_name = self.expect_ident("a parameter name")?;
                parameters.push(Parameter { name: parameter_name, ty });
                if self.at(&TokenKind::Comma) {
                    self.bump();
                } else {
                    break;
                }
            }
            if self.at(&TokenKind::Arrow) {
                self.bump();
                return_type = Some(self.type_()?);
            }
        }
        self.expect(&TokenKind::RParen, "`)` closing the function header")?;
        self.expect(&TokenKind::LBrace, "`{` opening the function body")?;
        let body = self.block(&[TokenKind::RBrace])?;
        self.expect(&TokenKind::RBrace, "`}` to close the function body")?;
        self.expect_semi("the function declaration")?;
        Ok(Stmt::Function { name, parameters, return_type, body })
    }

    pub(super) fn import_statement(&mut self) -> Result<Stmt, Diagnostic> {
        self.bump();
        let path = match self.peek() {
            Some(TokenKind::Str(_)) => {
                let TokenKind::Str(path) = self.bump().kind else {
                    unreachable!("matched Str")
                };
                path
            }
            _ => return Err(self.expected("a module path string after `import`")),
        };
        let alias = if self.at(&TokenKind::As) {
            self.bump();
            Some(self.expect_ident("an alias name after `as`")?)
        } else {
            None
        };
        self.expect_semi("the `import` statement")?;
        Ok(Stmt::Import { path, alias })
    }

    pub(super) fn terminal_statement(&mut self) -> Result<Stmt, Diagnostic> {
        self.bump();
        let word = self.expect_ident("a command name after `.`")?;
        if word != "terminal" {
            return Err(self.error_here(format!(
                "unknown dot statement `.{word}`, expected `.terminal`"
            )));
        }
        self.expect(&TokenKind::Bang, "`!` before the terminal command")?;
        let command = self.expect_ident("a terminal command name")?;
        self.expect_semi("the terminal command")?;
        Ok(Stmt::Terminal { command })
    }

    pub(super) fn type_(&mut self) -> Result<Type, Diagnostic> {
        let name = self.expect_ident("a type name")?;
        let ty = match name.as_str() {
            "int" => Type::Int,
            "float" => Type::Float,
            "str" => Type::Str,
            "boolean" => Type::Boolean,
            "date" => Type::Date,
            "json" => Type::Json,
            "set" => Type::Set,
            "table" => Type::Table,
            "map" => {
                self.expect(&TokenKind::ColonColon, "`::` after `map`")?;
                let key = self.type_()?;
                self.expect(&TokenKind::Bridge, "`<->` between the map key and value types")?;
                let value = self.type_()?;
                Type::Map { key: Box::new(key), value: Box::new(value) }
            }
            other => {
                return Err(self.error_here(format!(
                    "unknown type `{other}`, expected int, float, str, boolean, date, json, set, table or map"
                )));
            }
        };
        Ok(ty)
    }
}
