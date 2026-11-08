use super::Parser;
use crate::diagnostics::Diagnostic;
use crate::frontend::ast::{BinaryOp, Expr, Expression, MapEntry, Spanned, UnaryOp};
use crate::frontend::token::TokenKind;

impl Parser {
    pub(super) fn expression(&mut self) -> Result<Expression, Diagnostic> {
        let value = self.or_level()?;
        if self.at(&TokenKind::From) {
            self.bump();
            let source = self.or_level()?;
            let location = value.location;
            return Ok(Spanned {
                node: Expr::Binary {
                    op: BinaryOp::From,
                    left: Box::new(value),
                    right: Box::new(source),
                },
                location,
            });
        }
        Ok(value)
    }

    fn or_level(&mut self) -> Result<Expression, Diagnostic> {
        let mut left = self.set_level()?;
        while self.at(&TokenKind::Or) {
            self.bump();
            let right = self.set_level()?;
            let location = left.location;
            left = Spanned {
                node: Expr::Binary {
                    op: BinaryOp::Or,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                location,
            };
        }
        Ok(left)
    }

    fn set_level(&mut self) -> Result<Expression, Diagnostic> {
        let mut left = self.and_level()?;
        loop {
            let op = match self.peek() {
                Some(TokenKind::Union) => BinaryOp::Union,
                Some(TokenKind::Intersection) => BinaryOp::Intersection,
                Some(TokenKind::Difference) => BinaryOp::Difference,
                Some(TokenKind::SymmetricDifference) => BinaryOp::SymmetricDifference,
                _ => break,
            };
            self.bump();
            let right = self.and_level()?;
            let location = left.location;
            left = Spanned {
                node: Expr::Binary { op, left: Box::new(left), right: Box::new(right) },
                location,
            };
        }
        Ok(left)
    }

    fn and_level(&mut self) -> Result<Expression, Diagnostic> {
        let mut left = self.comparison_level()?;
        while self.at(&TokenKind::And) {
            self.bump();
            let right = self.comparison_level()?;
            let location = left.location;
            left = Spanned {
                node: Expr::Binary {
                    op: BinaryOp::And,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                location,
            };
        }
        Ok(left)
    }

    fn comparison_level(&mut self) -> Result<Expression, Diagnostic> {
        let mut left = self.additive_level()?;
        loop {
            let op = match self.peek() {
                Some(TokenKind::Eq) => BinaryOp::Eq,
                Some(TokenKind::NotEq) => BinaryOp::NotEq,
                Some(TokenKind::Lt) => BinaryOp::Lt,
                Some(TokenKind::Gt) => BinaryOp::Gt,
                Some(TokenKind::Le) => BinaryOp::Le,
                Some(TokenKind::Ge) => BinaryOp::Ge,
                Some(TokenKind::Has) => BinaryOp::Has,
                _ => break,
            };
            self.bump();
            let right = self.additive_level()?;
            let location = left.location;
            left = Spanned {
                node: Expr::Binary { op, left: Box::new(left), right: Box::new(right) },
                location,
            };
        }
        Ok(left)
    }

    fn additive_level(&mut self) -> Result<Expression, Diagnostic> {
        let mut left = self.multiplicative_level()?;
        loop {
            let op = match self.peek() {
                Some(TokenKind::Plus) => BinaryOp::Add,
                Some(TokenKind::Minus) => BinaryOp::Sub,
                Some(TokenKind::Concat) => BinaryOp::Concat,
                _ => break,
            };
            self.bump();
            let right = self.multiplicative_level()?;
            let location = left.location;
            left = Spanned {
                node: Expr::Binary { op, left: Box::new(left), right: Box::new(right) },
                location,
            };
        }
        Ok(left)
    }

    fn multiplicative_level(&mut self) -> Result<Expression, Diagnostic> {
        let mut left = self.unary_level()?;
        loop {
            let op = match self.peek() {
                Some(TokenKind::Star) => BinaryOp::Mul,
                Some(TokenKind::Slash) => BinaryOp::Div,
                Some(TokenKind::Percent) => BinaryOp::Mod,
                _ => break,
            };
            self.bump();
            let right = self.unary_level()?;
            let location = left.location;
            left = Spanned {
                node: Expr::Binary { op, left: Box::new(left), right: Box::new(right) },
                location,
            };
        }
        Ok(left)
    }

    fn unary_level(&mut self) -> Result<Expression, Diagnostic> {
        let location = self.current_location();
        if self.at(&TokenKind::Minus) {
            self.bump();
            let operand = self.unary_level()?;
            return Ok(Spanned {
                node: Expr::Unary { op: UnaryOp::Neg, operand: Box::new(operand) },
                location,
            });
        }
        if self.at(&TokenKind::Not) {
            self.bump();
            let operand = self.unary_level()?;
            return Ok(Spanned {
                node: Expr::Unary { op: UnaryOp::Not, operand: Box::new(operand) },
                location,
            });
        }
        self.postfix_level()
    }

    fn postfix_level(&mut self) -> Result<Expression, Diagnostic> {
        let primary = self.primary()?;
        self.postfix_from(primary)
    }

    pub(super) fn postfix_from(&mut self, mut expr: Expression) -> Result<Expression, Diagnostic> {
        loop {
            let location = expr.location;
            match self.peek() {
                Some(TokenKind::Dot) => {
                    self.bump();
                    let field = self.expect_member_name("a member name after `.`")?;
                    if self.at(&TokenKind::LParen) {
                        self.bump();
                        let arguments = self.arguments(&TokenKind::RParen)?;
                        self.expect(&TokenKind::RParen, "`)` closing the call arguments")?;
                        expr = Spanned {
                            node: Expr::MethodCall {
                                receiver: Box::new(expr),
                                method: field,
                                arguments,
                            },
                            location,
                        };
                    } else {
                        expr = Spanned {
                            node: Expr::FieldAccess {
                                receiver: Box::new(expr),
                                field,
                            },
                            location,
                        };
                    }
                }
                Some(TokenKind::LBracket) => {
                    self.bump();
                    let index = self.expression()?;
                    self.expect(&TokenKind::RBracket, "`]` closing the index")?;
                    expr = Spanned {
                        node: Expr::Index {
                            receiver: Box::new(expr),
                            index: Box::new(index),
                        },
                        location,
                    };
                }
                Some(TokenKind::LParen)
                    if matches!(&expr.node, Expr::Ident(_) | Expr::FieldAccess { .. }) =>
                {
                    let call_target = match &expr.node {
                        Expr::Ident(name) => (None, name.clone()),
                        Expr::FieldAccess { receiver, field } => {
                            (Some(receiver.clone()), field.clone())
                        }
                        _ => unreachable!("guarded by the match condition"),
                    };
                    self.bump();
                    let arguments = self.arguments(&TokenKind::RParen)?;
                    self.expect(&TokenKind::RParen, "`)` closing the call arguments")?;
                    expr = match call_target {
                        (Some(receiver), method) => Spanned {
                            node: Expr::MethodCall { receiver, method, arguments },
                            location,
                        },
                        (None, callee) => Spanned {
                            node: Expr::Call { callee, arguments },
                            location,
                        },
                    };
                }
                Some(TokenKind::ColonColon) if matches!(&expr.node, Expr::Ident(_)) => {
                    let Expr::Ident(callee) = &expr.node else {
                        unreachable!("guarded by the match condition")
                    };
                    let callee = callee.clone();
                    self.bump();
                    let arguments = self.arguments(&TokenKind::Semi)?;
                    expr = Spanned {
                        node: Expr::Call { callee, arguments },
                        location,
                    };
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    fn arguments(&mut self, terminator: &TokenKind) -> Result<Vec<Expression>, Diagnostic> {
        let mut arguments = Vec::new();
        if self.at(terminator) {
            return Ok(arguments);
        }
        loop {
            arguments.push(self.expression()?);
            if self.at(&TokenKind::Comma) {
                self.bump();
            } else {
                break;
            }
        }
        Ok(arguments)
    }

    fn primary(&mut self) -> Result<Expression, Diagnostic> {
        let location = self.current_location();
        let node = match self.peek() {
            Some(TokenKind::Int(_)) => {
                let TokenKind::Int(value) = self.bump().kind else {
                    unreachable!("matched Int")
                };
                Expr::Int(value)
            }
            Some(TokenKind::Float(_)) => {
                let TokenKind::Float(value) = self.bump().kind else {
                    unreachable!("matched Float")
                };
                Expr::Float(value)
            }
            Some(TokenKind::Str(_)) => {
                let TokenKind::Str(value) = self.bump().kind else {
                    unreachable!("matched Str")
                };
                Expr::Str(value)
            }
            Some(TokenKind::Char(_)) => {
                let TokenKind::Char(value) = self.bump().kind else {
                    unreachable!("matched Char")
                };
                Expr::Char(value)
            }
            Some(TokenKind::True) => {
                self.bump();
                Expr::Bool(true)
            }
            Some(TokenKind::False) => {
                self.bump();
                Expr::Bool(false)
            }
            Some(TokenKind::RawBlockOpen) => {
                self.bump();
                let TokenKind::RawText(content) = self.bump().kind else {
                    unreachable!("the lexer emits raw text between the raw block delimiters");
                };
                self.expect(&TokenKind::RawBlockClose, "`>>>` closing the raw block")?;
                Expr::RawJson(content)
            }
            Some(TokenKind::Ident(_)) => Expr::Ident(self.expect_ident("an expression")?),
            Some(TokenKind::LParen) => {
                self.bump();
                let inner = self.expression()?;
                self.expect(&TokenKind::RParen, "`)` closing the parenthesized expression")?;
                return Ok(inner);
            }
            Some(TokenKind::LBrace) => {
                self.bump();
                let mut entries = Vec::new();
                while !self.at(&TokenKind::RBrace) {
                    let key = self.expression()?;
                    self.expect(&TokenKind::ColonColon, "`::` between the map key and value")?;
                    let value = self.expression()?;
                    entries.push(MapEntry { key, value });
                    if self.at(&TokenKind::Comma) {
                        self.bump();
                    } else {
                        break;
                    }
                }
                self.expect(&TokenKind::RBrace, "`}` closing the map literal")?;
                Expr::MapLiteral(entries)
            }
            Some(TokenKind::LBracket) => {
                self.bump();
                let mut elements = Vec::new();
                while !self.at(&TokenKind::RBracket) {
                    elements.push(self.expression()?);
                    if self.at(&TokenKind::Comma) {
                        self.bump();
                    } else {
                        break;
                    }
                }
                self.expect(&TokenKind::RBracket, "`]` closing the array literal")?;
                Expr::Array(elements)
            }
            _ => return Err(self.expected("an expression")),
        };
        Ok(Spanned { node, location })
    }
}
