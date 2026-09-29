use crate::analyzer::{analyze, ValueType};
use crate::ir::{lower, ExprKind, StatementKind};
use crate::source::{SourceFile, SourceSet};

#[test]
fn lowers_checked_arithmetic_to_typed_ir() {
    let mut sources = SourceSet::default();
    sources
        .insert(
            SourceFile::new(
                "core/arithmetic.ubi",
                include_bytes!("../examples/core/arithmetic.ubi").to_vec(),
            )
            .unwrap(),
        )
        .unwrap();
    let analysis = analyze(&sources);
    assert!(
        analysis.diagnostics.is_empty(),
        "{:#?}",
        analysis.diagnostics
    );

    let program = lower(&analysis).unwrap();
    let function = program
        .functions
        .get(&("core/arithmetic.ubi".to_owned(), "evaluate".to_owned()))
        .unwrap();
    assert_eq!(function.return_type, ValueType::Int);
    let expression = function.body.tail.as_ref().unwrap();
    assert_eq!(expression.ty, ValueType::Int);
    assert!(matches!(expression.kind, ExprKind::Binary { .. }));

    let minimum = program
        .functions
        .get(&("core/arithmetic.ubi".to_owned(), "minimum".to_owned()))
        .unwrap();
    assert!(matches!(
        minimum.body.tail.as_ref().unwrap().kind,
        ExprKind::Integer(i32::MIN)
    ));
}

#[test]
fn lowers_imported_named_calls_to_resolved_function_keys() {
    let mut sources = SourceSet::default();
    for (id, bytes) in [
        (
            "modules/app/main.ubi",
            include_bytes!("../examples/modules/app/main.ubi").as_slice(),
        ),
        (
            "modules/lib/math.ubi",
            include_bytes!("../examples/modules/lib/math.ubi").as_slice(),
        ),
    ] {
        sources
            .insert(SourceFile::new(id, bytes.to_vec()).unwrap())
            .unwrap();
    }
    let analysis = analyze(&sources);
    assert!(
        analysis.diagnostics.is_empty(),
        "{:#?}",
        analysis.diagnostics
    );

    let program = lower(&analysis).unwrap();
    let function = program
        .functions
        .get(&("modules/app/main.ubi".to_owned(), "run".to_owned()))
        .unwrap();
    let call = function.body.tail.as_ref().unwrap();
    let ExprKind::Call { target, .. } = &call.kind else {
        panic!("expected a named call");
    };
    assert_eq!(
        target,
        &("modules/lib/math.ubi".to_owned(), "double".to_owned())
    );
}

#[test]
fn lowers_returning_if_branch_and_keeps_other_branch_value() {
    let mut sources = SourceSet::default();
    sources
        .insert(
            SourceFile::new(
                "core/control.ubi",
                include_bytes!("../examples/core/control.ubi").to_vec(),
            )
            .unwrap(),
        )
        .unwrap();
    let analysis = analyze(&sources);
    assert!(
        analysis.diagnostics.is_empty(),
        "{:#?}",
        analysis.diagnostics
    );

    let program = lower(&analysis).unwrap();
    let function = program
        .functions
        .get(&("core/control.ubi".to_owned(), "branch".to_owned()))
        .unwrap();
    let StatementKind::Let { value, .. } = &function.body.statements[0].kind else {
        panic!("expected the if expression initializer");
    };
    let ExprKind::If {
        then_branch,
        else_branch,
        ..
    } = &value.kind
    else {
        panic!("expected the if expression");
    };
    assert_eq!(value.ty, ValueType::Int);
    assert!(matches!(
        then_branch.statements[0].kind,
        StatementKind::Return(Some(_))
    ));
    assert!(matches!(else_branch.kind, ExprKind::Block(_)));
}

#[test]
fn folds_only_the_direct_minimum_integer_literal() {
    let mut sources = SourceSet::default();
    sources
        .insert(
            SourceFile::new(
                "main.ubi",
                b"export fn negative() -> int { -7 }\nexport fn minimum() -> int { -2147483648 }\nexport fn negativeZero() -> float { -0.0 }".to_vec(),
            )
            .unwrap(),
        )
        .unwrap();
    let analysis = analyze(&sources);
    assert!(
        analysis.diagnostics.is_empty(),
        "{:#?}",
        analysis.diagnostics
    );
    let program = lower(&analysis).unwrap();

    let negative = program
        .functions
        .get(&("main.ubi".to_owned(), "negative".to_owned()))
        .unwrap();
    let ExprKind::Unary { operand, .. } = &negative.body.tail.as_ref().unwrap().kind else {
        panic!("ordinary negative literals retain their unary expression");
    };
    assert!(matches!(operand.kind, ExprKind::Integer(7)));
    let minimum = program
        .functions
        .get(&("main.ubi".to_owned(), "minimum".to_owned()))
        .unwrap();
    assert!(matches!(
        minimum.body.tail.as_ref().unwrap().kind,
        ExprKind::Integer(i32::MIN)
    ));
    let negative_zero = program
        .functions
        .get(&("main.ubi".to_owned(), "negativeZero".to_owned()))
        .unwrap();
    let ExprKind::Unary { operand, .. } = &negative_zero.body.tail.as_ref().unwrap().kind else {
        panic!("negative float literals retain their unary expression");
    };
    assert_eq!(operand.ty, ValueType::Float);
    assert!(matches!(operand.kind, ExprKind::Float(value) if value == 0.0));
}
