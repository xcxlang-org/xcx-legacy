use crate::diagnostics::Location;
use crate::frontend::ast::{Expr, Expression, Statement, Stmt};

pub(super) struct Flow {
    pub fallthrough: bool,
    pub bare_return: Option<Location>,
    breaks: bool,
}

pub(super) fn analyze(body: &[Statement]) -> Flow {
    let mut flow = Flow { fallthrough: true, bare_return: None, breaks: false };
    for statement in body {
        if !flow.fallthrough {
            break;
        }
        let next = statement_flow(statement);
        flow.fallthrough = next.fallthrough;
        flow.breaks |= next.breaks;
        flow.bare_return = flow.bare_return.or(next.bare_return);
    }
    flow
}

pub(crate) fn is_endless_loop(condition: &Expression, body: &[Statement]) -> bool {
    matches!(condition.node, Expr::Bool(true)) && !analyze(body).breaks
}

fn statement_flow(statement: &Statement) -> Flow {
    let mut flow = Flow { fallthrough: true, bare_return: None, breaks: false };
    match &statement.node {
        Stmt::Return { value } => {
            flow.fallthrough = false;
            flow.bare_return = value.is_none().then_some(statement.location);
        }
        Stmt::Halt | Stmt::Continue => flow.fallthrough = false,
        Stmt::Terminal { command } if command == "exit" => flow.fallthrough = false,
        Stmt::Break => {
            flow.fallthrough = false;
            flow.breaks = true;
        }
        Stmt::If { branches, otherwise } => {
            flow.fallthrough = otherwise.is_none();
            for body in branches.iter().map(|branch| &branch.body).chain(otherwise.iter()) {
                let branch = analyze(body);
                flow.fallthrough |= branch.fallthrough;
                flow.breaks |= branch.breaks;
                flow.bare_return = flow.bare_return.or(branch.bare_return);
            }
        }
        Stmt::While { condition, body } => {
            let body = analyze(body);
            flow.fallthrough = !matches!(condition.node, Expr::Bool(true)) || body.breaks;
            flow.bare_return = body.bare_return;
        }
        Stmt::ForRange { body, .. } | Stmt::ForIn { body, .. } => {
            flow.bare_return = analyze(body).bare_return;
        }
        _ => {}
    }
    flow
}
