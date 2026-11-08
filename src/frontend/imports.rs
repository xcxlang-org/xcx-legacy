use crate::diagnostics::Diagnostic;
use crate::frontend::ast::{Expr, Spanned, Statement, Stmt, Type};
use std::path::{Path, PathBuf};

pub fn expand(program: Vec<Statement>) -> Result<Vec<Statement>, Diagnostic> {
    expand_with(program, Path::new("."), &mut Vec::new())
}

fn expand_with(
    program: Vec<Statement>,
    base: &Path,
    stack: &mut Vec<PathBuf>,
) -> Result<Vec<Statement>, Diagnostic> {
    let mut result = Vec::new();
    for statement in program {
        if let Stmt::Import { path, alias } = &statement.node {
            result.extend(load_import(path, alias.as_deref(), base, stack, statement.location)?);
        } else {
            result.push(statement);
        }
    }
    Ok(result)
}

fn load_import(
    path: &str,
    alias: Option<&str>,
    base: &Path,
    stack: &mut Vec<PathBuf>,
    location: crate::diagnostics::Location,
) -> Result<Vec<Statement>, Diagnostic> {
    let full = base.join(path);
    let canonical = full.canonicalize().map_err(|error| {
        Diagnostic::new(
            format!("cannot read import `{path}`: {error}"),
            location,
        )
    })?;
    if stack.contains(&canonical) {
        let mut chain: Vec<String> = stack
            .iter()
            .map(|entry| entry.display().to_string())
            .collect();
        chain.push(canonical.display().to_string());
        return Err(Diagnostic::new(
            format!("circular import detected: {}", chain.join(" -> ")),
            location,
        ));
    }
    let source = std::fs::read_to_string(&canonical).map_err(|error| {
        Diagnostic::new(format!("cannot read import `{path}`: {error}"), location)
    })?;

    if path.ends_with(".json") {
        let Some(alias) = alias else {
            return Err(Diagnostic::new(
                format!("the json import `{path}` requires an alias (`as name`)"),
                location,
            ));
        };
        return Ok(vec![Spanned {
            node: Stmt::Const {
                name: alias.to_string(),
                ty: Type::Str,
                init: Spanned {
                    node: Expr::Str(source),
                    location,
                },
            },
            location,
        }]);
    }

    let tokens = crate::frontend::lexer::tokenize(&source).map_err(|error| {
        Diagnostic::new(format!("in `{path}`: {}", error.message), error.location)
    })?;
    let imported = crate::frontend::parser::parse(tokens).map_err(|error| {
        Diagnostic::new(format!("in `{path}`: {}", error.message), error.location)
    })?;
    stack.push(canonical.clone());
    let expanded = expand_with(
        imported,
        canonical.parent().unwrap_or_else(|| Path::new(".")),
        stack,
    )
    .map_err(|error| Diagnostic::new(format!("in `{path}`: {}", error.message), error.location))?;
    stack.pop();
    Ok(expanded)
}
