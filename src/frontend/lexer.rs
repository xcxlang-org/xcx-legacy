use crate::diagnostics::{Diagnostic, Location};
use crate::frontend::token::{Token, TokenKind};

pub fn tokenize(source: &str) -> Result<Vec<Token>, Diagnostic> {
    Lexer::new(source).run()
}

struct Lexer {
    characters: Vec<char>,
    index: usize,
    location: Location,
}

impl Lexer {
    fn new(source: &str) -> Self {
        Self {
            characters: source.chars().collect(),
            index: 0,
            location: Location { line: 1, column: 1 },
        }
    }

    fn run(mut self) -> Result<Vec<Token>, Diagnostic> {
        let mut tokens = Vec::new();
        while let Some(character) = self.peek() {
            if character.is_whitespace() {
                self.advance();
                continue;
            }
            let start = self.location;
            if character == '#' {
                self.comment(start)?;
                continue;
            }
            if character.is_ascii_digit() {
                tokens.push(self.number(start)?);
                continue;
            }
            if character == '"' {
                tokens.push(self.string(start)?);
                continue;
            }
            if character == '\'' {
                tokens.push(self.character_literal(start)?);
                continue;
            }
            if character == '<' && self.peek_at(1) == Some('<') && self.peek_at(2) == Some('<') {
                tokens.extend(self.raw_block(start)?);
                continue;
            }
            if character.is_ascii_alphabetic() || character == '_' {
                tokens.push(self.identifier(start));
                continue;
            }
            if let Some(kind) = self.operator() {
                tokens.push(Token { kind, location: start });
                continue;
            }
            return Err(Diagnostic::new(format!("unexpected character `{character}`"), start));
        }
        Ok(tokens)
    }

    fn peek(&self) -> Option<char> {
        self.characters.get(self.index).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<char> {
        self.characters.get(self.index + offset).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let character = self.peek()?;
        if character == '\n' {
            self.location.line += 1;
            self.location.column = 1;
        } else {
            self.location.column += 1;
        }
        self.index += 1;
        Some(character)
    }

    fn comment(&mut self, start: Location) -> Result<(), Diagnostic> {
        self.advance();
        if self.peek() == Some('#') {
            self.advance();
            while let Some(character) = self.peek() {
                if character == '#' && self.peek_at(1) == Some('#') {
                    self.advance();
                    self.advance();
                    return Ok(());
                }
                self.advance();
            }
            return Err(Diagnostic::new("unterminated multi-line comment", start));
        }
        while let Some(character) = self.peek() {
            if character == '\n' {
                break;
            }
            self.advance();
        }
        Ok(())
    }

    fn number(&mut self, start: Location) -> Result<Token, Diagnostic> {
        let mut digits = String::new();
        while matches!(self.peek(), Some(character) if character.is_ascii_digit()) {
            digits.push(self.advance().expect("peeked character"));
        }
        if self.peek() == Some('.') {
            match self.peek_at(1) {
                Some(next) if next.is_ascii_digit() => {
                    digits.push(self.advance().expect("peeked character"));
                    while matches!(self.peek(), Some(character) if character.is_ascii_digit()) {
                        digits.push(self.advance().expect("peeked character"));
                    }
                    let value = digits.parse::<f64>().map_err(|_| {
                        Diagnostic::new(format!("invalid float literal `{digits}`"), start)
                    })?;
                    return Ok(Token { kind: TokenKind::Float(value), location: start });
                }
                _ => {
                    let location = self.location;
                    self.advance();
                    return Err(Diagnostic::new(
                        "expected a digit after the decimal point",
                        location,
                    ));
                }
            }
        }
        let value = digits.parse::<i64>().map_err(|_| {
            Diagnostic::new(format!("integer literal `{digits}` does not fit in i64"), start)
        })?;
        Ok(Token { kind: TokenKind::Int(value), location: start })
    }

    fn string(&mut self, start: Location) -> Result<Token, Diagnostic> {
        self.advance();
        let mut text = String::new();
        loop {
            match self.peek() {
                None | Some('\n') => return Err(Diagnostic::new("unterminated string literal", start)),
                Some('"') => {
                    self.advance();
                    return Ok(Token { kind: TokenKind::Str(text), location: start });
                }
                Some('\\') => {
                    let escape_location = self.location;
                    self.advance();
                    let escape = self.escape(escape_location)?;
                    text.push(escape);
                }
                Some(_) => text.push(self.advance().expect("peeked character")),
            }
        }
    }

    fn character_literal(&mut self, start: Location) -> Result<Token, Diagnostic> {
        self.advance();
        let value = match self.peek() {
            Some('\\') => {
                let escape_location = self.location;
                self.advance();
                self.escape(escape_location)?
            }
            Some(_) => self.advance().expect("peeked character"),
            None => return Err(Diagnostic::new("unterminated character literal", start)),
        };
        if self.peek() != Some('\'') {
            return Err(Diagnostic::new("unterminated character literal", start));
        }
        self.advance();
        Ok(Token { kind: TokenKind::Char(value), location: start })
    }

    fn escape(&mut self, backslash_location: Location) -> Result<char, Diagnostic> {
        let character = match self.advance() {
            Some(character) => character,
            None => return Err(Diagnostic::new("unterminated escape sequence", backslash_location)),
        };
        Ok(match character {
            'n' => '\n',
            't' => '\t',
            'r' => '\r',
            '0' => '\0',
            '"' => '"',
            '\'' => '\'',
            '\\' => '\\',
            other => {
                return Err(Diagnostic::new(
                    format!("unknown escape sequence `\\{other}`"),
                    backslash_location,
                ));
            }
        })
    }

    fn raw_block(&mut self, start: Location) -> Result<Vec<Token>, Diagnostic> {
        self.advance();
        self.advance();
        self.advance();
        let mut text = String::new();
        loop {
            match self.peek() {
                None => return Err(Diagnostic::new("unterminated raw block", start)),
                Some('>') if self.peek_at(1) == Some('>') && self.peek_at(2) == Some('>') => {
                    let content = Token { kind: TokenKind::RawText(text), location: start };
                    let open = Token { kind: TokenKind::RawBlockOpen, location: start };
                    let close_location = self.location;
                    self.advance();
                    self.advance();
                    self.advance();
                    let close = Token { kind: TokenKind::RawBlockClose, location: close_location };
                    return Ok(vec![open, content, close]);
                }
                Some(_) => text.push(self.advance().expect("peeked character")),
            }
        }
    }

    fn identifier(&mut self, start: Location) -> Token {
        let mut text = String::new();
        while matches!(self.peek(), Some(character) if character.is_ascii_alphanumeric() || character == '_')
        {
            text.push(self.advance().expect("peeked character"));
        }
        let kind = match text.as_str() {
            "let" => TokenKind::Let,
            "const" => TokenKind::Const,
            "dec" => TokenKind::Dec,
            "func" => TokenKind::Func,
            "if" => TokenKind::If,
            "elf" => TokenKind::Elf,
            "els" => TokenKind::Els,
            "then" => TokenKind::Then,
            "end" => TokenKind::End,
            "for" => TokenKind::For,
            "while" => TokenKind::While,
            "do" => TokenKind::Do,
            "to" => TokenKind::To,
            "in" => TokenKind::In,
            "from" => TokenKind::From,
            "return" => TokenKind::Return,
            "break" => TokenKind::Break,
            "continue" => TokenKind::Continue,
            "halt" => TokenKind::Halt,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "AND" => TokenKind::And,
            "OR" => TokenKind::Or,
            "NOT" => TokenKind::Not,
            "has" => TokenKind::Has,
            "as" => TokenKind::As,
            "import" => TokenKind::Import,
            "UNION" => TokenKind::Union,
            "INTERSECTION" => TokenKind::Intersection,
            "DIFFERENCE" => TokenKind::Difference,
            "SYMMETRIC_DIFFERENCE" => TokenKind::SymmetricDifference,
            _ => TokenKind::Ident(text),
        };
        Token { kind, location: start }
    }

    fn operator(&mut self) -> Option<TokenKind> {
        match self.peek()? {
            '+' => match self.peek_at(1) {
                Some('+') => {
                    self.advance();
                    self.advance();
                    Some(TokenKind::Concat)
                }
                _ => single(self, TokenKind::Plus),
            },
            '-' => match self.peek_at(1) {
                Some('>') => {
                    self.advance();
                    self.advance();
                    Some(TokenKind::Arrow)
                }
                _ => single(self, TokenKind::Minus),
            },
            '*' => single(self, TokenKind::Star),
            '/' => single(self, TokenKind::Slash),
            '%' => single(self, TokenKind::Percent),
            '=' => match self.peek_at(1) {
                Some('=') => {
                    self.advance();
                    self.advance();
                    Some(TokenKind::Eq)
                }
                _ => single(self, TokenKind::Assign),
            },
            '!' => match self.peek_at(1) {
                Some('=') => {
                    self.advance();
                    self.advance();
                    Some(TokenKind::NotEq)
                }
                _ => single(self, TokenKind::Bang),
            },
            '<' => match (self.peek_at(1), self.peek_at(2)) {
                (Some('-'), Some('>')) => {
                    self.advance();
                    self.advance();
                    self.advance();
                    Some(TokenKind::Bridge)
                }
                (Some('='), _) => {
                    self.advance();
                    self.advance();
                    Some(TokenKind::Le)
                }
                _ => single(self, TokenKind::Lt),
            },
            '>' => match self.peek_at(1) {
                Some('!') => {
                    self.advance();
                    self.advance();
                    Some(TokenKind::PrintOp)
                }
                Some('?') => {
                    self.advance();
                    self.advance();
                    Some(TokenKind::ReadOp)
                }
                Some('=') => {
                    self.advance();
                    self.advance();
                    Some(TokenKind::Ge)
                }
                _ => single(self, TokenKind::Gt),
            },
            ':' => match self.peek_at(1) {
                Some(':') => {
                    self.advance();
                    self.advance();
                    Some(TokenKind::ColonColon)
                }
                _ => None,
            },
            '(' => single(self, TokenKind::LParen),
            ')' => single(self, TokenKind::RParen),
            '{' => single(self, TokenKind::LBrace),
            '}' => single(self, TokenKind::RBrace),
            '[' => single(self, TokenKind::LBracket),
            ']' => single(self, TokenKind::RBracket),
            ',' => single(self, TokenKind::Comma),
            '.' => single(self, TokenKind::Dot),
            ';' => single(self, TokenKind::Semi),
            '@' => single(self, TokenKind::At),
            _ => None,
        }
    }
}

fn single(lexer: &mut Lexer, kind: TokenKind) -> Option<TokenKind> {
    lexer.advance();
    Some(kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(source: &str) -> Vec<TokenKind> {
        tokenize(source)
            .expect("source must tokenize")
            .into_iter()
            .map(|token| token.kind)
            .collect()
    }

    fn error(source: &str) -> Diagnostic {
        tokenize(source).expect_err("source must be rejected")
    }

    #[test]
    fn empty_source_yields_no_tokens() {
        assert_eq!(tokenize("").expect("empty source"), Vec::new());
    }

    #[test]
    fn lowercase_keywords_are_recognized() {
        let source = "let const dec func if elf els then end for while do to in from \
                      return break continue halt true false has as import";
        assert_eq!(
            kinds(source),
            vec![
                TokenKind::Let,
                TokenKind::Const,
                TokenKind::Dec,
                TokenKind::Func,
                TokenKind::If,
                TokenKind::Elf,
                TokenKind::Els,
                TokenKind::Then,
                TokenKind::End,
                TokenKind::For,
                TokenKind::While,
                TokenKind::Do,
                TokenKind::To,
                TokenKind::In,
                TokenKind::From,
                TokenKind::Return,
                TokenKind::Break,
                TokenKind::Continue,
                TokenKind::Halt,
                TokenKind::True,
                TokenKind::False,
                TokenKind::Has,
                TokenKind::As,
                TokenKind::Import,
            ]
        );
    }

    #[test]
    fn uppercase_keywords_are_recognized() {
        assert_eq!(
            kinds("AND OR NOT UNION INTERSECTION DIFFERENCE SYMMETRIC_DIFFERENCE"),
            vec![
                TokenKind::And,
                TokenKind::Or,
                TokenKind::Not,
                TokenKind::Union,
                TokenKind::Intersection,
                TokenKind::Difference,
                TokenKind::SymmetricDifference,
            ]
        );
    }

    #[test]
    fn capitalized_identifiers_are_not_keywords() {
        assert_eq!(
            kinds("For LET union"),
            vec![
                TokenKind::Ident("For".into()),
                TokenKind::Ident("LET".into()),
                TokenKind::Ident("union".into()),
            ]
        );
    }

    #[test]
    fn single_character_operators_are_recognized() {
        assert_eq!(
            kinds("+ - * / % = < > ( ) { } [ ] , . ; @ !"),
            vec![
                TokenKind::Plus,
                TokenKind::Minus,
                TokenKind::Star,
                TokenKind::Slash,
                TokenKind::Percent,
                TokenKind::Assign,
                TokenKind::Lt,
                TokenKind::Gt,
                TokenKind::LParen,
                TokenKind::RParen,
                TokenKind::LBrace,
                TokenKind::RBrace,
                TokenKind::LBracket,
                TokenKind::RBracket,
                TokenKind::Comma,
                TokenKind::Dot,
                TokenKind::Semi,
                TokenKind::At,
                TokenKind::Bang,
            ]
        );
    }

    #[test]
    fn multi_character_operators_are_recognized() {
        assert_eq!(
            kinds("++ -> == != <= >= :: >! >? <->"),
            vec![
                TokenKind::Concat,
                TokenKind::Arrow,
                TokenKind::Eq,
                TokenKind::NotEq,
                TokenKind::Le,
                TokenKind::Ge,
                TokenKind::ColonColon,
                TokenKind::PrintOp,
                TokenKind::ReadOp,
                TokenKind::Bridge,
            ]
        );
    }

    #[test]
    fn number_float_and_char_literals_are_recognized() {
        assert_eq!(
            kinds("5 19.99 3.0 'x' -7"),
            vec![
                TokenKind::Int(5),
                TokenKind::Float(19.99),
                TokenKind::Float(3.0),
                TokenKind::Char('x'),
                TokenKind::Minus,
                TokenKind::Int(7),
            ]
        );
    }

    #[test]
    fn string_escapes_are_decoded() {
        assert_eq!(
            kinds("\"a\\n\\t\\r\\\"\\\\b\""),
            vec![TokenKind::Str("a\n\t\r\"\\b".into())]
        );
    }

    #[test]
    fn char_literal_escapes_are_decoded() {
        assert_eq!(kinds("'\\n'"), vec![TokenKind::Char('\n')]);
    }

    #[test]
    fn raw_block_text_spans_lines_verbatim() {
        let tokens = tokenize("<<<\n{\"k\": 1}\n>>>").expect("raw block");
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0].kind, TokenKind::RawBlockOpen);
        assert_eq!(tokens[1].kind, TokenKind::RawText("\n{\"k\": 1}\n".into()));
        assert_eq!(tokens[2].kind, TokenKind::RawBlockClose);
    }

    #[test]
    fn comments_produce_no_tokens() {
        assert_eq!(kinds("# line\n##\nblock\n##\n42"), vec![TokenKind::Int(42)]);
    }

    #[test]
    fn positions_count_characters_not_bytes() {
        let tokens = tokenize("\"ąą\" >!").expect("source must tokenize");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[1].kind, TokenKind::PrintOp);
        assert_eq!(tokens[1].location, Location { line: 1, column: 6 });
    }

    #[test]
    fn float_without_a_digit_after_the_point_is_rejected() {
        let diagnostic = error("1.;");
        assert_eq!(diagnostic.location.line, 1);
        assert_eq!(diagnostic.location.column, 2);
        assert_eq!(diagnostic.message, "expected a digit after the decimal point");
    }

    #[test]
    fn integer_overflow_is_rejected() {
        let diagnostic = error("99999999999999999999");
        assert_eq!(diagnostic.location.line, 1);
        assert_eq!(diagnostic.location.column, 1);
        assert!(diagnostic.message.contains("does not fit in i64"), "{diagnostic}");
    }

    #[test]
    fn unknown_escape_is_rejected_at_the_backslash() {
        let diagnostic = error("\"a\\qb\"");
        assert_eq!(diagnostic.location.line, 1);
        assert_eq!(diagnostic.location.column, 3);
        assert_eq!(diagnostic.message, "unknown escape sequence `\\q`");
    }

    #[test]
    fn unterminated_string_is_rejected_at_its_opening() {
        let diagnostic = error("\"abc");
        assert_eq!(diagnostic.location, Location { line: 1, column: 1 });
        assert_eq!(diagnostic.message, "unterminated string literal");
    }

    #[test]
    fn string_may_not_span_lines() {
        let diagnostic = error("\"ab\nc\"");
        assert_eq!(diagnostic.location.line, 1);
        assert_eq!(diagnostic.message, "unterminated string literal");
    }

    #[test]
    fn unterminated_character_literal_is_rejected() {
        let diagnostic = error("'x");
        assert_eq!(diagnostic.message, "unterminated character literal");
    }

    #[test]
    fn unterminated_block_comment_is_rejected_at_its_opening() {
        let diagnostic = error("## never closed");
        assert_eq!(diagnostic.location.line, 1);
        assert_eq!(diagnostic.location.column, 1);
        assert_eq!(diagnostic.message, "unterminated multi-line comment");
    }

    #[test]
    fn unterminated_raw_block_is_rejected_at_its_opening() {
        let diagnostic = error("<<< never closed");
        assert_eq!(diagnostic.location.line, 1);
        assert_eq!(diagnostic.location.column, 1);
        assert_eq!(diagnostic.message, "unterminated raw block");
    }

    #[test]
    fn lone_colon_is_rejected() {
        let diagnostic = error("x : y");
        assert_eq!(diagnostic.location.column, 3);
        assert!(diagnostic.message.contains("`:`"), "{diagnostic}");
    }
}
