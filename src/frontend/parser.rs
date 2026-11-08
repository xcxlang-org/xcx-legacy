use crate::diagnostics::{Diagnostic, Location};
use crate::frontend::ast::{Expr, Spanned, Statement, Stmt};
use crate::frontend::token::{Token, TokenKind};

mod declarations;
mod expressions;
mod flow;

#[cfg(test)]
#[path = "parser/tests.rs"]
mod tests;

pub fn parse(tokens: Vec<Token>) -> Result<Vec<Statement>, Diagnostic> {
    Parser::new(tokens).parse_program()
}

struct Parser {
    tokens: Vec<Token>,
    position: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, position: 0 }
    }

    fn parse_program(mut self) -> Result<Vec<Statement>, Diagnostic> {
        let mut statements = Vec::new();
        while !self.at_end() {
            statements.push(self.statement()?);
        }
        Ok(statements)
    }

    fn at_end(&self) -> bool {
        self.position >= self.tokens.len()
    }

    fn peek(&self) -> Option<&TokenKind> {
        self.tokens.get(self.position).map(|token| &token.kind)
    }

    fn peek_at(&self, offset: usize) -> Option<&TokenKind> {
        self.tokens.get(self.position + offset).map(|token| &token.kind)
    }

    fn current_location(&self) -> Location {
        self.tokens
            .get(self.position)
            .or_else(|| self.tokens.last())
            .map(|token| token.location)
            .unwrap_or(Location { line: 1, column: 1 })
    }

    fn bump(&mut self) -> Token {
        let token = self.tokens[self.position].clone();
        self.position += 1;
        token
    }

    fn at(&self, kind: &TokenKind) -> bool {
        self.peek() == Some(kind)
    }

    fn error_here(&self, message: impl Into<String>) -> Diagnostic {
        Diagnostic::new(message, self.current_location())
    }

    fn expected(&self, what: &str) -> Diagnostic {
        let found = match self.peek() {
            Some(kind) => kind.describe(),
            None => "end of file".to_string(),
        };
        self.error_here(format!("expected {what}, found {found}"))
    }

    fn expect(&mut self, kind: &TokenKind, what: &str) -> Result<Token, Diagnostic> {
        if self.at(kind) {
            Ok(self.bump())
        } else {
            Err(self.expected(what))
        }
    }

    fn expect_semi(&mut self, after: &str) -> Result<(), Diagnostic> {
        self.expect(&TokenKind::Semi, &format!("`;` after {after}"))?;
        Ok(())
    }

    fn expect_ident(&mut self, what: &str) -> Result<String, Diagnostic> {
        match self.peek() {
            Some(TokenKind::Ident(_)) => {
                let TokenKind::Ident(name) = self.bump().kind else {
                    unreachable!("matched Ident")
                };
                Ok(name)
            }
            _ => Err(self.expected(what)),
        }
    }

    fn expect_member_name(&mut self, what: &str) -> Result<String, Diagnostic> {
        match self.peek() {
            Some(TokenKind::Ident(_)) => {
                let TokenKind::Ident(name) = self.bump().kind else {
                    unreachable!("matched Ident")
                };
                Ok(name)
            }
            Some(kind) => match kind.keyword_spelling() {
                Some(spelling) => {
                    self.bump();
                    Ok(spelling.to_string())
                }
                None => Err(self.expected(what)),
            },
            None => Err(self.expected(what)),
        }
    }

    fn statement(&mut self) -> Result<Statement, Diagnostic> {
        let location = self.current_location();
        let node = match self.peek() {
            Some(TokenKind::Let) => self.let_statement()?,
            Some(TokenKind::Const) => self.const_statement()?,
            Some(TokenKind::Dec) => self.function_statement()?,
            Some(TokenKind::If) => self.if_statement()?,
            Some(TokenKind::While) => self.while_statement()?,
            Some(TokenKind::For) => self.for_statement()?,
            Some(TokenKind::Return) => self.return_statement()?,
            Some(TokenKind::Import) => self.import_statement()?,
            Some(TokenKind::Break) => {
                self.bump();
                self.expect_semi("`break`")?;
                Stmt::Break
            }
            Some(TokenKind::Continue) => {
                self.bump();
                self.expect_semi("`continue`")?;
                Stmt::Continue
            }
            Some(TokenKind::Halt) => {
                self.bump();
                self.expect_semi("`halt`")?;
                Stmt::Halt
            }
            Some(TokenKind::PrintOp) => {
                self.bump();
                let value = self.expression()?;
                self.expect_semi("the `>!` expression")?;
                Stmt::Print { value }
            }
            Some(TokenKind::ReadOp) => {
                self.bump();
                let name = self.expect_ident("a variable name after `>?`")?;
                self.expect_semi("the `>?` target")?;
                Stmt::Read { name }
            }
            Some(TokenKind::Dot) => self.terminal_statement()?,
            Some(TokenKind::Ident(_)) => self.name_statement()?,
            _ => return Err(self.expected("a statement")),
        };
        Ok(Spanned { node, location })
    }

    fn name_statement(&mut self) -> Result<Stmt, Diagnostic> {
        let location = self.current_location();
        let name = self.expect_ident("an identifier")?;
        if self.at(&TokenKind::Assign) {
            self.bump();
            let value = self.expression()?;
            self.expect_semi("the assignment value")?;
            return Ok(Stmt::Assign { name, value });
        }
        let expr = self.postfix_from(Spanned { node: Expr::Ident(name), location })?;
        self.expect_semi("the expression statement")?;
        Ok(Stmt::ExpressionStatement { expr })
    }

    fn block(&mut self, terminators: &[TokenKind]) -> Result<Vec<Statement>, Diagnostic> {
        let mut statements = Vec::new();
        loop {
            let Some(kind) = self.peek() else {
                let names: Vec<String> =
                    terminators.iter().map(|kind| kind.describe()).collect();
                return Err(self.error_here(format!(
                    "expected {} before the end of file",
                    names.join(" or ")
                )));
            };
            if terminators.contains(kind) {
                return Ok(statements);
            }
            statements.push(self.statement()?);
        }
    }
}
