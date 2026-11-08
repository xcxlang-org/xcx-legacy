use super::*;
use crate::frontend::ast::{BinaryOp, SetElement, Type};
use crate::frontend::lexer;

fn program(source: &str) -> Vec<Statement> {
    let tokens = lexer::tokenize(source).expect("source must tokenize");
    parse(tokens).expect("source must parse")
}

fn error(source: &str) -> Diagnostic {
    let tokens = lexer::tokenize(source).expect("source must tokenize");
    parse(tokens).expect_err("source must be rejected")
}

#[test]
fn empty_source_parses_to_no_statements() {
    assert_eq!(program(""), Vec::new());
}

#[test]
fn let_statement_accepts_multiple_bindings() {
    let statements = program("let (int) x = 1, y = 2, z = 3;");
    let Stmt::Let { ty, bindings } = &statements[0].node else {
        panic!("expected a let statement");
    };
    assert_eq!(*ty, Type::Int);
    assert_eq!(bindings.len(), 3);
    assert_eq!(bindings[2].name, "z");
}

#[test]
fn let_statement_accepts_a_declaration_without_initializer() {
    let statements = program("let (str) text;");
    assert!(matches!(statements[0].node, Stmt::Let { ref bindings, .. } if bindings.len() == 1 && bindings[0].init.is_none()));
}

#[test]
fn mixed_initialized_and_uninitialized_bindings_are_rejected() {
    let diagnostic = error("let (int) x = 1, y;");
    assert!(diagnostic.message.contains("must be initialized"), "{diagnostic}");
}

#[test]
fn const_statement_requires_an_initializer() {
    let diagnostic = error("const (int) MAX;");
    assert_eq!(diagnostic.message, "constants require an initializer");
}

#[test]
fn unknown_type_is_rejected() {
    let diagnostic = error("let (banana) x = 1;");
    assert!(diagnostic.message.contains("unknown type `banana`"), "{diagnostic}");
}

#[test]
fn map_type_requires_the_bridge_operator() {
    let diagnostic = error("let (map :: str -> int) m = {};");
    assert!(diagnostic.message.contains("`<->`"), "{diagnostic}");
}

#[test]
fn if_statement_collects_elf_branches_and_the_else_block() {
    let source = "if (a) then; >! 1; elf (b) then; >! 2; els; >! 3; end;";
    let statements = program(source);
    let Stmt::If { branches, otherwise } = &statements[0].node else {
        panic!("expected an if statement");
    };
    assert_eq!(branches.len(), 2);
    assert!(otherwise.is_some());
}

#[test]
fn if_statement_without_end_is_rejected() {
    let diagnostic = error("if (a) then; >! 1;");
    assert!(diagnostic.message.contains("`end`"), "{diagnostic}");
}

#[test]
fn for_statement_parses_both_forms() {
    let source = "for i in 1 to 5; >! i; end; for (str) k in m.keys(); >! k; end;";
    let statements = program(source);
    assert!(matches!(statements[0].node, Stmt::ForRange { ref variable, .. } if variable == "i"));
    assert!(matches!(statements[1].node, Stmt::ForIn { ref variable, .. } if variable == "k"));
}

#[test]
fn while_statement_parses_with_its_body() {
    let statements = program("while (x < 3) do; x = x + 1; end;");
    let Stmt::While { body, .. } = &statements[0].node else {
        panic!("expected a while statement");
    };
    assert_eq!(body.len(), 1);
}

#[test]
fn function_declaration_parses_parameters_and_return_type() {
    let source = "dec func (add :: let (int) a, let (int) b -> int) { return a + b; };";
    let statements = program(source);
    let Stmt::Function { parameters, return_type, .. } = &statements[0].node else {
        panic!("expected a function declaration");
    };
    assert_eq!(parameters.len(), 2);
    assert_eq!(*return_type, Some(Type::Int));
}

#[test]
fn call_binds_tighter_than_addition() {
    let source = ">! \"n! = \" + factorial :: 5;";
    let statements = program(source);
    let Stmt::Print { value } = &statements[0].node else {
        panic!("expected a print statement");
    };
    let Expr::Binary { op, left, right } = &value.node else {
        panic!("expected a binary expression");
    };
    assert_eq!(*op, BinaryOp::Add);
    assert!(matches!(left.node, Expr::Str(_)));
    assert!(matches!(right.node, Expr::Call { ref callee, .. } if callee == "factorial"));
}

#[test]
fn set_declaration_parses_ranges_and_values() {
    let statements = program("let (set) ranges { N = 1,,5, 10,,15 };");
    let Stmt::LetSet { elements, .. } = &statements[0].node else {
        panic!("expected a set declaration");
    };
    assert_eq!(elements.len(), 2);
    assert!(matches!(elements[0], SetElement::Range { step: None, .. }));
    assert!(matches!(elements[1], SetElement::Range { step: None, .. }));

    let statements = program("let (set) colors { S = \"a\", \"b\" };");
    let Stmt::LetSet { elements, .. } = &statements[0].node else {
        panic!("expected a set declaration");
    };
    assert!(matches!(elements[0], SetElement::Value(_)));
    assert!(matches!(elements[1], SetElement::Value(_)));
}

#[test]
fn set_declaration_parses_a_stepped_range() {
    let statements = program("let (set) even { N = 2,,20,,2 };");
    let Stmt::LetSet { elements, .. } = &statements[0].node else {
        panic!("expected a set declaration");
    };
    assert!(matches!(elements[0], SetElement::Range { step: Some(_), .. }));
}

#[test]
fn unknown_set_designator_is_rejected() {
    let diagnostic = error("let (set) x { X = 1 };");
    assert!(diagnostic.message.contains("unknown set designator"), "{diagnostic}");
}

#[test]
fn table_declaration_parses_columns_with_auto() {
    let source = "let (table) t { columns = [ id :: int @auto, name :: str ] rows = [ EMPTY ] };";
    let statements = program(source);
    let Stmt::LetTable { columns, .. } = &statements[0].node else {
        panic!("expected a table declaration");
    };
    assert_eq!(columns.len(), 2);
    assert!(columns[0].auto);
    assert!(!columns[1].auto);
}

#[test]
fn map_literals_parse_empty_and_filled() {
    let source = "let (map :: str <-> int) a = {}; let (map :: str <-> int) b = { \"x\" :: 1, \"y\" :: 2 };";
    let statements = program(source);
    let Stmt::Let { bindings, .. } = &statements[0].node else {
        panic!("expected a let statement");
    };
    assert!(matches!(
        bindings[0].init.as_ref().expect("initialized").node,
        Expr::MapLiteral(ref entries) if entries.is_empty()
    ));
    let Stmt::Let { bindings, .. } = &statements[1].node else {
        panic!("expected a let statement");
    };
    assert!(matches!(
        bindings[0].init.as_ref().expect("initialized").node,
        Expr::MapLiteral(ref entries) if entries.len() == 2
    ));
}

#[test]
fn raw_block_becomes_a_raw_json_expression() {
    let source = "let (json) data <<< {\"k\": 1} >>>;";
    let statements = program(source);
    let Stmt::Let { bindings, .. } = &statements[0].node else {
        panic!("expected a let statement");
    };
    let init = bindings[0].init.as_ref().expect("initialized");
    assert!(matches!(&init.node, Expr::RawJson(text) if text == " {\"k\": 1} "));
}

#[test]
fn postfix_chains_parse_field_access_index_and_method_calls() {
    let source = ">! sales[0].items.count();";
    let statements = program(source);
    let Stmt::Print { value } = &statements[0].node else {
        panic!("expected a print statement");
    };
    assert!(matches!(
        &value.node,
        Expr::MethodCall { method, .. } if method == "count"
    ));
}

#[test]
fn random_choice_parses_with_from() {
    let source = "let (int) x = random.choice from numbers;";
    let statements = program(source);
    let Stmt::Let { bindings, .. } = &statements[0].node else {
        panic!("expected a let statement");
    };
    let init = bindings[0].init.as_ref().expect("initialized");
    assert!(matches!(&init.node, Expr::Binary { op: BinaryOp::From, .. }));
}

#[test]
fn set_operations_parse_as_binary_expressions() {
    let source = "let (set) u = a UNION b; let (set) i = a INTERSECTION b;";
    let statements = program(source);
    assert!(matches!(
        &statements[0].node,
        Stmt::Let { bindings, .. } if matches!(&bindings[0].init, Some(e) if matches!(e.node, Expr::Binary { op: BinaryOp::Union, .. }))
    ));
    assert!(matches!(
        &statements[1].node,
        Stmt::Let { bindings, .. } if matches!(&bindings[0].init, Some(e) if matches!(e.node, Expr::Binary { op: BinaryOp::Intersection, .. }))
    ));
}

#[test]
fn terminal_statement_parses_its_command() {
    let statements = program(".terminal !help;");
    assert!(matches!(statements[0].node, Stmt::Terminal { ref command } if command == "help"));
}

#[test]
fn unknown_dot_statement_is_rejected() {
    let diagnostic = error(".print 1;");
    assert!(diagnostic.message.contains("unknown dot statement"), "{diagnostic}");
}

#[test]
fn missing_semicolon_is_positioned_at_the_offending_token() {
    let diagnostic = error("let (int) x = 1\n>! x;");
    assert_eq!(diagnostic.location.line, 2);
    assert!(diagnostic.message.contains("expected `;`"), "{diagnostic}");
}

#[test]
fn bare_identifier_statement_parses_as_an_expression_statement() {
    let statements = program("greeting;");
    assert!(matches!(statements[0].node, Stmt::ExpressionStatement { .. }));
}

#[test]
fn import_statement_parses_path_and_optional_alias() {
    let statements = program("import \"a.xcx\"; import \"lib/b.xcx\" as b;");
    assert!(matches!(&statements[0].node, Stmt::Import { path, alias } if path == "a.xcx" && alias.is_none()));
    assert!(matches!(&statements[1].node, Stmt::Import { alias: Some(_), .. }));
}
