use super::Parser;
use crate::diagnostics::Diagnostic;
use crate::frontend::ast::{Branch, Stmt};
use crate::frontend::token::TokenKind;

impl Parser {
    pub(super) fn if_statement(&mut self) -> Result<Stmt, Diagnostic> {
        self.bump();
        self.expect(&TokenKind::LParen, "`(` after `if`")?;
        let condition = self.expression()?;
        self.expect(&TokenKind::RParen, "`)` after the condition")?;
        self.expect(&TokenKind::Then, "`then` after the condition")?;
        self.expect_semi("`then`")?;
        let mut branches = vec![Branch {
            condition,
            body: self.block(&[TokenKind::Elf, TokenKind::Els, TokenKind::End])?,
        }];
        let mut otherwise = None;
        loop {
            match self.peek() {
                Some(TokenKind::Elf) => {
                    self.bump();
                    self.expect(&TokenKind::LParen, "`(` after `elf`")?;
                    let condition = self.expression()?;
                    self.expect(&TokenKind::RParen, "`)` after the condition")?;
                    self.expect(&TokenKind::Then, "`then` after the condition")?;
                    self.expect_semi("`then`")?;
                    branches.push(Branch {
                        condition,
                        body: self.block(&[TokenKind::Elf, TokenKind::Els, TokenKind::End])?,
                    });
                }
                Some(TokenKind::Els) => {
                    self.bump();
                    self.expect_semi("`els`")?;
                    otherwise = Some(self.block(&[TokenKind::End])?);
                    break;
                }
                _ => break,
            }
        }
        self.expect(&TokenKind::End, "`end` closing the `if` statement")?;
        self.expect_semi("`end`")?;
        Ok(Stmt::If { branches, otherwise })
    }

    pub(super) fn while_statement(&mut self) -> Result<Stmt, Diagnostic> {
        self.bump();
        self.expect(&TokenKind::LParen, "`(` after `while`")?;
        let condition = self.expression()?;
        self.expect(&TokenKind::RParen, "`)` after the condition")?;
        self.expect(&TokenKind::Do, "`do` after the condition")?;
        self.expect_semi("`do`")?;
        let body = self.block(&[TokenKind::End])?;
        self.expect(&TokenKind::End, "`end` closing the `while` loop")?;
        self.expect_semi("`end`")?;
        Ok(Stmt::While { condition, body })
    }

    pub(super) fn for_statement(&mut self) -> Result<Stmt, Diagnostic> {
        self.bump();
        if self.at(&TokenKind::LParen) {
            self.bump();
            let variable_type = self.type_()?;
            self.expect(&TokenKind::RParen, "`)` after the loop variable type")?;
            let variable = self.expect_ident("a loop variable name")?;
            self.expect(&TokenKind::In, "`in` after the loop variable")?;
            let iterable = self.expression()?;
            self.expect_semi("the `for … in` header")?;
            let body = self.block(&[TokenKind::End])?;
            self.expect(&TokenKind::End, "`end` closing the `for` loop")?;
            self.expect_semi("`end`")?;
            return Ok(Stmt::ForIn { variable_type, variable, iterable, body });
        }
        let variable = self.expect_ident("a loop variable name")?;
        self.expect(&TokenKind::In, "`in` after the loop variable")?;
        let start = self.expression()?;
        self.expect(&TokenKind::To, "`to` after the range start")?;
        let end = self.expression()?;
        self.expect_semi("the `for … to` header")?;
        let body = self.block(&[TokenKind::End])?;
        self.expect(&TokenKind::End, "`end` closing the `for` loop")?;
        self.expect_semi("`end`")?;
        Ok(Stmt::ForRange { variable, start, end, body })
    }

    pub(super) fn return_statement(&mut self) -> Result<Stmt, Diagnostic> {
        self.bump();
        let value = if self.at(&TokenKind::Semi) {
            None
        } else {
            Some(self.expression()?)
        };
        self.expect_semi("`return`")?;
        Ok(Stmt::Return { value })
    }
}
