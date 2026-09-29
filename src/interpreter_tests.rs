use crate::analyzer::{analyze, Analysis, FunctionKey};
use crate::interpreter::{invoke, InvocationError, Value};
use crate::source::{SourceFile, SourceSet};

fn checked(items: &[(&str, &[u8])]) -> Analysis {
    let mut sources = SourceSet::default();
    for (id, bytes) in items {
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
    analysis
}

fn run(
    analysis: &Analysis,
    module: &str,
    function: &str,
    arguments: Vec<Value>,
) -> Result<Value, InvocationError> {
    let key: FunctionKey = (module.to_owned(), function.to_owned());
    invoke(analysis, &key, arguments)
}

#[test]
fn refuses_to_interpret_programs_with_frontend_errors() {
    let mut sources = SourceSet::default();
    sources
        .insert(
            SourceFile::new(
                "main.ubi",
                b"export fn answer() -> int { missing }".to_vec(),
            )
            .unwrap(),
        )
        .unwrap();
    let analysis = analyze(&sources);

    assert!(!analysis.diagnostics.is_empty());
    assert_eq!(
        run(&analysis, "main.ubi", "answer", vec![]),
        Err(InvocationError::InvalidProgram)
    );
}

#[test]
fn evaluates_arithmetic_bindings_calls_and_early_returns() {
    let arithmetic = checked(&[(
        "core/arithmetic.ubi",
        include_bytes!("../examples/core/arithmetic.ubi"),
    )]);
    assert_eq!(
        run(&arithmetic, "core/arithmetic.ubi", "evaluate", vec![]),
        Ok(Value::Int(-15))
    );
    assert_eq!(
        run(&arithmetic, "core/arithmetic.ubi", "minimum", vec![]),
        Ok(Value::Int(i32::MIN))
    );
    assert_eq!(
        run(
            &arithmetic,
            "core/arithmetic.ubi",
            "minimumRemainder",
            vec![]
        ),
        Ok(Value::Int(0))
    );

    let bindings = checked(&[(
        "core/bindings.ubi",
        include_bytes!("../examples/core/bindings.ubi"),
    )]);
    assert_eq!(
        run(&bindings, "core/bindings.ubi", "shadow", vec![]),
        Ok(Value::Int(9))
    );
    assert_eq!(
        run(&bindings, "core/bindings.ubi", "discard", vec![]),
        Ok(Value::Unit)
    );
    assert_eq!(
        run(&bindings, "core/bindings.ubi", "unitEquality", vec![]),
        Ok(Value::Bool(true))
    );

    let control = checked(&[(
        "core/control.ubi",
        include_bytes!("../examples/core/control.ubi"),
    )]);
    assert_eq!(
        run(
            &control,
            "core/control.ubi",
            "factorial",
            vec![Value::Int(5)]
        ),
        Ok(Value::Int(120))
    );
    assert_eq!(
        run(
            &control,
            "core/control.ubi",
            "branch",
            vec![Value::Bool(true)]
        ),
        Ok(Value::Int(7))
    );
    assert_eq!(
        run(
            &control,
            "core/control.ubi",
            "branch",
            vec![Value::Bool(false)]
        ),
        Ok(Value::Int(9))
    );
}

#[test]
fn evaluates_binary64_special_values_and_comparisons() {
    let analysis = checked(&[(
        "core/floats.ubi",
        include_bytes!("../examples/core/floats.ubi"),
    )]);
    assert_eq!(
        run(&analysis, "core/floats.ubi", "finite", vec![]),
        Ok(Value::Float(1.5))
    );
    assert!(
        matches!(run(&analysis, "core/floats.ubi", "negativeZero", vec![]), Ok(Value::Float(v)) if v == 0.0 && v.is_sign_negative())
    );
    assert!(
        matches!(run(&analysis, "core/floats.ubi", "positiveInfinity", vec![]), Ok(Value::Float(v)) if v == f64::INFINITY)
    );
    assert!(
        matches!(run(&analysis, "core/floats.ubi", "negativeInfinity", vec![]), Ok(Value::Float(v)) if v == f64::NEG_INFINITY)
    );
    assert!(
        matches!(run(&analysis, "core/floats.ubi", "notANumber", vec![]), Ok(Value::Float(v)) if v.is_nan())
    );
    assert_eq!(
        run(&analysis, "core/floats.ubi", "rounding", vec![]),
        Ok(Value::Float(9_007_199_254_740_992.0))
    );
    assert!(
        matches!(run(&analysis, "core/floats.ubi", "underflow", vec![]), Ok(Value::Float(v)) if v == 0.0 && !v.is_sign_negative())
    );
    assert_eq!(
        run(&analysis, "core/floats.ubi", "comparisons", vec![]),
        Ok(Value::Bool(true))
    );
}

#[test]
fn resolves_imported_calls_and_preserves_short_circuit_and_fault_order() {
    let imports = checked(&[
        (
            "modules/app/main.ubi",
            include_bytes!("../examples/modules/app/main.ubi"),
        ),
        (
            "modules/lib/math.ubi",
            include_bytes!("../examples/modules/lib/math.ubi"),
        ),
    ]);
    assert_eq!(
        run(&imports, "modules/app/main.ubi", "run", vec![]),
        Ok(Value::Int(42))
    );

    let short_circuit = checked(&[(
        "core/short-circuit.ubi",
        include_bytes!("../examples/core/short-circuit.ubi"),
    )]);
    assert_eq!(
        run(&short_circuit, "core/short-circuit.ubi", "run", vec![]),
        Ok(Value::Bool(true))
    );

    let order = checked(&[(
        "faults/evaluation-order.ubi",
        include_bytes!("../examples/faults/evaluation-order.ubi"),
    )]);
    assert_fault(
        run(&order, "faults/evaluation-order.ubi", "arguments", vec![]),
        "UBI-R0002",
    );
    assert_fault(
        run(&order, "faults/evaluation-order.ubi", "operands", vec![]),
        "UBI-R0002",
    );
    assert_eq!(
        run(
            &order,
            "faults/evaluation-order.ubi",
            "selectedBranch",
            vec![]
        ),
        Ok(Value::Int(8))
    );
}

#[test]
fn reports_checked_integer_overflow_and_zero_divisors() {
    let overflow = checked(&[(
        "faults/overflow.ubi",
        include_bytes!("../examples/faults/overflow.ubi"),
    )]);
    for function in [
        "addition",
        "subtraction",
        "multiplication",
        "negation",
        "division",
    ] {
        assert_fault(
            run(&overflow, "faults/overflow.ubi", function, vec![]),
            "UBI-R0001",
        );
    }

    let zero = checked(&[(
        "faults/zero-divisor.ubi",
        include_bytes!("../examples/faults/zero-divisor.ubi"),
    )]);
    for function in ["division", "remainder"] {
        assert_fault(
            run(&zero, "faults/zero-divisor.ubi", function, vec![]),
            "UBI-R0002",
        );
    }
}

#[test]
fn rejects_invalid_host_entrypoints_and_argument_shapes_before_execution() {
    let analysis = checked(&[(
        "core/control.ubi",
        include_bytes!("../examples/core/control.ubi"),
    )]);
    let branch: FunctionKey = ("core/control.ubi".to_owned(), "branch".to_owned());
    assert_eq!(
        invoke(&analysis, &branch, vec![Value::Int(1)]),
        Err(InvocationError::InvalidArguments)
    );

    let bindings = checked(&[(
        "core/bindings.ubi",
        include_bytes!("../examples/core/bindings.ubi"),
    )]);
    let private: FunctionKey = ("core/bindings.ubi".to_owned(), "twice".to_owned());
    assert_eq!(
        invoke(&bindings, &private, vec![Value::Int(1)]),
        Err(InvocationError::NotExported)
    );
    let missing: FunctionKey = ("core/control.ubi".to_owned(), "missing".to_owned());
    assert_eq!(
        invoke(&analysis, &missing, vec![]),
        Err(InvocationError::UnknownExport)
    );
}

#[test]
fn interprets_direct_minimum_integer_with_intervening_comment() {
    let analysis = checked(&[(
        "main.ubi",
        b"export fn minimum() -> int { - /* gap */ 2147483648 }",
    )]);
    assert_eq!(
        run(&analysis, "main.ubi", "minimum", vec![]),
        Ok(Value::Int(i32::MIN))
    );
}

#[test]
fn recursion_and_total_work_are_bounded_by_runtime_faults() {
    let deep = checked(&[(
        "main.ubi",
        b"export fn recurse(value: int) -> int { recurse(value + 1) }",
    )]);
    assert_fault(
        run(&deep, "main.ubi", "recurse", vec![Value::Int(0)]),
        "UBI-R0005",
    );

    let work = checked(&[(
        "main.ubi",
        b"export fn work(value: int) -> int { if (value <= 0) { 0 } else { work(value - 1) + work(value - 1) } }",
    )]);
    assert_fault(
        run(&work, "main.ubi", "work", vec![Value::Int(30)]),
        "UBI-R0005",
    );
}

fn assert_fault(result: Result<Value, InvocationError>, code: &str) {
    let Err(InvocationError::Runtime(fault)) = result else {
        panic!("expected runtime fault {code}, got {result:?}");
    };
    assert_eq!(fault.code, code);
    assert!(!fault.message.is_empty());
}
