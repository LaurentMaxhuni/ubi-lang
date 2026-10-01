use crate::lexer::Symbol;
use crate::parser::{parse, Declaration, ExprKind, StatementKind};

#[test]
fn parses_all_milestone_one_core_programs() {
    for (id, source) in [
        (
            "arithmetic.ubi",
            include_str!("../examples/core/arithmetic.ubi"),
        ),
        (
            "bindings.ubi",
            include_str!("../examples/core/bindings.ubi"),
        ),
        ("control.ubi", include_str!("../examples/core/control.ubi")),
        ("floats.ubi", include_str!("../examples/core/floats.ubi")),
        (
            "short-circuit.ubi",
            include_str!("../examples/core/short-circuit.ubi"),
        ),
    ] {
        parse(id, source).unwrap_or_else(|error| panic!("{id}: {error:?}"));
    }
}

#[test]
fn pratt_parser_observes_precedence_and_byte_spans() {
    let module = parse("main.ubi", "export fn value() -> int { 1 + 2 * 3 }").unwrap();
    let Declaration::Function(function) = &module.declarations[0] else {
        panic!("expected function")
    };
    let expression = function.body.tail.as_ref().unwrap();
    assert_eq!((expression.span.start, expression.span.end), (27, 36));
    let ExprKind::Binary {
        operator: Symbol::Plus,
        right,
        ..
    } = &expression.kind
    else {
        panic!("expected an addition at the expression root");
    };
    assert!(matches!(
        right.kind,
        ExprKind::Binary {
            operator: Symbol::Star,
            ..
        }
    ));
}

#[test]
fn let_and_return_statements_keep_their_syntax_spans() {
    let module = parse("main.ubi", "fn f() -> int { let x: int = 2; return x; }").unwrap();
    let Declaration::Function(function) = &module.declarations[0] else {
        panic!("expected function")
    };
    assert!(matches!(
        function.body.statements[0].kind,
        StatementKind::Let { .. }
    ));
    assert!(matches!(
        function.body.statements[1].kind,
        StatementKind::Return(Some(_))
    ));
}

#[test]
fn missing_tokens_report_ubi0002_at_eof() {
    let error = parse("main.ubi", "fn f() -> int { 1 +").unwrap_err();
    assert_eq!(error.code, "UBI0002");
    assert_eq!((error.primary.start, error.primary.end), (19, 19));
}

#[test]
fn imports_comments_and_trailing_commas_are_preserved() {
    let source =
        "// retained\nimport { value, } from \"./helper.ubi\";\nfn f(x: int,) { value(x) }";
    let module = parse("pkg/main.ubi", source).unwrap();
    assert_eq!(module.imports.len(), 1);
    assert_eq!(module.imports[0].path, "./helper.ubi");
    assert_eq!(module.imports[0].names[0].name, "value");
    assert_eq!(
        &source[module.imports[0].path_span.start..module.imports[0].path_span.end],
        "\"./helper.ubi\""
    );
    assert_eq!(module.comments.len(), 1);
}

#[test]
fn unsupported_keyword_and_later_profile_syntax_get_ubi0003() {
    let source = "fn f() { await task; }";
    let error = parse("main.ubi", source).unwrap_err();
    assert_eq!(error.code, "UBI0003");
    assert_eq!(&source[error.primary.start..error.primary.end], "await");

    let source = "enum Later {}";
    let error = parse("main.ubi", source).unwrap_err();
    assert_eq!(error.code, "UBI0003");
    assert_eq!(&source[error.primary.start..error.primary.end], "enum");
}

#[test]
fn parser_enforces_token_and_nesting_budgets() {
    let source = format!("fn f() {{ {}1{} }}", "(".repeat(130), ")".repeat(130));
    let error = parse("deep.ubi", &source).unwrap_err();
    assert_eq!(error.code, "UBI0090");

    let source = format!("fn f() {{ {}1 }}", "1+".repeat(250_000));
    let error = parse("wide.ubi", &source).unwrap_err();
    assert_eq!(error.code, "UBI0090");

    let source = format!("fn f() {{ 1{} }}", "+1".repeat(130));
    match parse("deep-tree.ubi", &source) {
        Err(error) => assert_eq!(error.code, "UBI0090"),
        Ok(module) => {
            std::mem::forget(module);
            panic!("accepted an expression AST over the nesting limit");
        }
    }
}

#[test]
fn parser_expression_spans_count_utf8_bytes() {
    let source = "fn f() -> string { \"é\" }";
    let module = parse("unicode.ubi", source).unwrap();
    let Declaration::Function(function) = &module.declarations[0] else {
        panic!("expected function")
    };
    let expression = function.body.tail.as_ref().unwrap();
    assert_eq!(&source[expression.span.start..expression.span.end], "\"é\"");
}

#[test]
fn comparison_operators_cannot_chain_but_logical_branches_can_compare() {
    let source = "fn f() -> bool { 1 < 2 == true }";
    let error = parse("main.ubi", source).unwrap_err();
    assert_eq!(error.code, "UBI0002");
    assert_eq!(&source[error.primary.start..error.primary.end], "==");

    parse("main.ubi", "fn f() -> bool { 1 < 2 && 3 < 4 }").unwrap();
    parse("main.ubi", "fn f() -> bool { 1 < (2 < 3) }").unwrap();
}

#[test]
fn parses_import_graph_and_reports_frontend_corpus_errors_in_the_right_phase() {
    let main = include_str!("../examples/modules/app/main.ubi");
    let library = include_str!("../examples/modules/lib/math.ubi");
    let main_module = parse("modules/app/main.ubi", main).unwrap();
    let library_module = parse("modules/lib/math.ubi", library).unwrap();
    assert_eq!(main_module.imports[0].path, "../lib/./math.ubi");
    assert_eq!(library_module.declarations.len(), 1);

    parse(
        "invalid.ubi",
        include_str!("../examples/invalid/unknown-unicode.ubi"),
    )
    .unwrap();
    parse(
        "invalid.ubi",
        include_str!("../examples/invalid/mixed-numeric.ubi"),
    )
    .unwrap();
    parse(
        "invalid.ubi",
        include_str!("../examples/invalid/immutable-binding.ubi"),
    )
    .unwrap();
    parse(
        "invalid.ubi",
        include_str!("../examples/invalid/call-arity.ubi"),
    )
    .unwrap();

    let unsupported = include_str!("../examples/invalid/unsupported-async.ubi");
    let error = parse("invalid.ubi", unsupported).unwrap_err();
    assert_eq!(error.code, "UBI0003");
    assert_eq!(
        &unsupported[error.primary.start..error.primary.end],
        "async"
    );

    let unterminated = include_str!("../examples/invalid/unterminated-string.ubi");
    let error = parse("invalid.ubi", unterminated).unwrap_err();
    assert_eq!(error.code, "UBI0001");
    assert_eq!(
        &unterminated[error.primary.start..error.primary.end],
        "\"unfinished"
    );

    let unexpected_eof = include_str!("../examples/invalid/unexpected-eof.ubi");
    let error = parse("invalid.ubi", unexpected_eof).unwrap_err();
    assert_eq!(error.code, "UBI0002");
    assert_eq!(error.primary.start, unexpected_eof.len());
    assert_eq!(error.primary.end, unexpected_eof.len());
}
