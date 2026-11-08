use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Location {
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub message: String,
    pub location: Location,
}

impl Diagnostic {
    pub fn new(message: impl Into<String>, location: Location) -> Self {
        Self {
            message: message.into(),
            location,
        }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "error {}:{}: {}",
            self.location.line, self.location.column, self.message
        )
    }
}

impl std::error::Error for Diagnostic {}
