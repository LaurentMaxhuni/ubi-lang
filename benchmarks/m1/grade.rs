// Public, independently derived M1 benchmark oracle. No external crates.
use crate::analyzer::analyze;
use crate::interpreter::{invoke, InvocationError, Value};
use crate::parser::{Block, Declaration, Expr, ExprKind, StatementKind};
use crate::source::{SourceFile, SourceSet};
use std::{fs, path::PathBuf, process::Command};

#[derive(Debug)]
enum Expected {
    Value(Value),
    Fault(&'static str),
}

struct Case {
    source: &'static str,
    function: &'static str,
    args: Vec<Value>,
    expected: Expected,
}

fn cases() -> Vec<Case> {
    use Value::Int as I;
    let mut cases = Vec::new();
    let mut add = |source, function, args, expected| {
        cases.push(Case {
            source,
            function,
            args,
            expected,
        });
    };
    for (input, result) in [
        (0, 1),
        (-1, 0),
        (i32::MIN, -2147483647),
        (i32::MAX - 1, i32::MAX),
        (42, 43),
    ] {
        add(
            "repair/main.ubi",
            "increment",
            vec![I(input)],
            Expected::Value(I(result)),
        );
    }
    add(
        "repair/main.ubi",
        "increment",
        vec![I(i32::MAX)],
        Expected::Fault("UBI-R0001"),
    );
    for (q, p, fee, result) in [
        (2, 3, 4, 10),
        (1, -3, 2, -1),
        (1, 0, i32::MIN, i32::MIN),
        (i32::MAX, 1, 0, i32::MAX),
        (0, i32::MAX, i32::MAX, 0),
        (-1, i32::MIN, i32::MAX, 0),
        (i32::MIN, i32::MIN, i32::MIN, 0),
        (2, -3, 4, -2),
    ] {
        add(
            "modules/main.ubi",
            "total",
            vec![I(q), I(p), I(fee)],
            Expected::Value(I(result)),
        );
    }
    for (q, p, fee) in [
        (2, i32::MAX, 0),
        (i32::MAX, 2, i32::MIN),
        (1, i32::MAX, 1),
        (1, i32::MIN, -1),
    ] {
        add(
            "modules/main.ubi",
            "total",
            vec![I(q), I(p), I(fee)],
            Expected::Fault("UBI-R0001"),
        );
    }
    for (q, p, result) in [
        (3, 4, 12),
        (-3, 4, -12),
        (0, i32::MIN, 0),
        (i32::MIN, 1, i32::MIN),
        (i32::MAX, 1, i32::MAX),
    ] {
        add(
            "modules/math.ubi",
            "subtotal",
            vec![I(q), I(p)],
            Expected::Value(I(result)),
        );
    }
    add(
        "modules/math.ubi",
        "subtotal",
        vec![I(i32::MIN), I(-1)],
        Expected::Fault("UBI-R0001"),
    );
    for (n, d, fallback, result) in [
        (7, 3, 99, 2),
        (-7, 3, 99, -2),
        (7, -3, 99, -2),
        (-7, -3, 99, 2),
        (1, 2, 99, 0),
        (-1, 2, 99, 0),
        (i32::MIN, 1, 99, i32::MIN),
        (i32::MAX, 1, 99, i32::MAX),
        (i32::MIN, 0, i32::MAX, i32::MAX),
        (i32::MAX, 0, i32::MIN, i32::MIN),
        (0, 0, 37, 37),
        (0, -1, 99, 0),
    ] {
        add(
            "division/main.ubi",
            "divideOr",
            vec![I(n), I(d), I(fallback)],
            Expected::Value(I(result)),
        );
    }
    add(
        "division/main.ubi",
        "divideOr",
        vec![I(i32::MIN), I(-1), I(99)],
        Expected::Fault("UBI-R0001"),
    );
    for title in ["", "Plan", "雪😀", "e\u{301}\n\t\"\\\0"] {
        for completed in [false, true] {
            let prefix = if completed { "[done] " } else { "[todo] " };
            add(
                "labels/main.ubi",
                "label",
                vec![Value::String(title.into()), Value::Bool(completed)],
                Expected::Value(Value::String(format!("{prefix}{title}"))),
            );
        }
    }
    assert_eq!(cases.len(), 45);
    cases
}

fn analysis() -> crate::analyzer::Analysis {
    let mut sources = SourceSet::default();
    for id in [
        "repair/main.ubi",
        "modules/main.ubi",
        "modules/math.ubi",
        "division/main.ubi",
        "labels/main.ubi",
    ] {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("benchmarks/m1")
            .join(id);
        sources
            .insert(SourceFile::new(id, fs::read(path).expect("benchmark source missing")).unwrap())
            .unwrap();
    }
    let analysis = analyze(&sources);
    assert!(
        analysis.diagnostics.is_empty(),
        "benchmark compile diagnostics: {:#?}",
        analysis.diagnostics
    );
    analysis
}

fn block_return(block: &Block, inside_if: bool) -> bool {
    block
        .statements
        .iter()
        .any(|statement| match &statement.kind {
            StatementKind::Return(_) if inside_if => true,
            StatementKind::Return(Some(expr)) | StatementKind::Expression(expr) => {
                expr_return(expr, inside_if)
            }
            StatementKind::Let { value, .. } | StatementKind::Assign { value, .. } => {
                expr_return(value, inside_if)
            }
            _ => false,
        })
        || block
            .tail
            .as_deref()
            .is_some_and(|expr| expr_return(expr, inside_if))
}

fn expr_return(expr: &Expr, inside_if: bool) -> bool {
    match &expr.kind {
        ExprKind::If {
            then_branch,
            else_branch,
            ..
        } => block_return(then_branch, true) || expr_return(else_branch, true),
        ExprKind::Block(block) => block_return(block, inside_if),
        ExprKind::Unary { operand, .. } | ExprKind::Propagate(operand) => {
            expr_return(operand, inside_if)
        }
        ExprKind::Binary { left, right, .. } => {
            expr_return(left, inside_if) || expr_return(right, inside_if)
        }
        ExprKind::Call { callee, arguments } => {
            expr_return(callee, inside_if)
                || arguments.iter().any(|arg| expr_return(arg, inside_if))
        }
        _ => false,
    }
}

#[test]
fn benchmark_grade_source_interpreter() {
    let analysis = analysis();
    let mut failed = Vec::new();
    for (index, case) in cases().into_iter().enumerate() {
        let actual = invoke(
            &analysis,
            &(case.source.into(), case.function.into()),
            case.args,
        );
        let passed = match (&case.expected, &actual) {
            (Expected::Value(expected), Ok(value)) => expected == value,
            (Expected::Fault(code), Err(InvocationError::Runtime(fault))) => {
                *code == fault.code && !fault.message.is_empty()
            }
            _ => false,
        };
        if !passed {
            failed.push(format!(
                "case {} {}: expected {:?}, got {:?}",
                index + 1,
                case.function,
                case.expected,
                actual
            ));
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
    println!("AST interpreter: 45/45 independent expectations passed");
}

#[test]
fn benchmark_grade_explicit_early_return() {
    let analysis = analysis();
    let function = analysis.modules["labels/main.ubi"]
        .declarations
        .iter()
        .find_map(|declaration| {
            let Declaration::Function(function) = declaration else {
                return None;
            };
            (function.name.name == "label").then_some(function)
        })
        .unwrap();
    assert!(
        block_return(&function.body, false),
        "label requires an explicit return inside an if branch"
    );
}

fn javascript(value: &Value) -> String {
    match value {
        Value::Int(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        Value::String(value) => {
            let mut result = String::from("\"");
            for scalar in value.chars() {
                result.push_str(&format!("\\u{{{:x}}}", scalar as u32));
            }
            result.push('"');
            result
        }
        _ => panic!("unsupported benchmark value"),
    }
}

struct Directory(PathBuf);
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn benchmark_grade_generated_javascript() {
    let analysis = analysis();
    let output = crate::js_backend::emit(&crate::ir::lower(&analysis).unwrap()).unwrap();
    let temp_path = std::env::temp_dir().join(format!("ubi-m1-benchmark-{}", std::process::id()));
    fs::create_dir(&temp_path).expect("unique benchmark temporary directory");
    let directory = Directory(temp_path);
    for (id, source) in output {
        let path = directory.0.join(id);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, source).unwrap();
    }
    let mut harness = String::from("import assert from 'node:assert/strict';\n");
    let ids = [
        "repair/main.ubi",
        "modules/main.ubi",
        "modules/math.ubi",
        "division/main.ubi",
        "labels/main.ubi",
    ];
    for (index, id) in ids.iter().enumerate() {
        harness.push_str(&format!(
            "import * as m{index} from './{}';\n",
            id.replace(".ubi", ".mjs")
        ));
    }
    harness.push_str("const failed = [];\n");
    for (index, case) in cases().into_iter().enumerate() {
        let module = ids.iter().position(|id| *id == case.source).unwrap();
        let args = case
            .args
            .iter()
            .map(javascript)
            .collect::<Vec<_>>()
            .join(",");
        let call = format!("m{module}.{}({args})", case.function);
        let assertion = match case.expected {
            Expected::Value(Value::Int(value)) => format!(
                "{{ const actual = {call}; assert.ok(Number.isInteger(actual) && actual >= -2147483648 && actual <= 2147483647 && actual === {value}); }}"
            ),
            Expected::Value(value) => format!("assert.equal({call}, {});", javascript(&value)),
            Expected::Fault(code) => format!("assert.throws(() => {call}, error => error.code === '{code}' && typeof error.message === 'string' && error.message.length > 0);"),
        };
        harness.push_str(&format!("try {{ {assertion} }} catch (error) {{ failed.push('case {} {}: ' + error.message); }}\n", index + 1, case.function));
    }
    harness.push_str("assert.deepEqual(failed, []);\nconsole.log('Generated JavaScript: 45/45 independent expectations passed');\n");
    let path = directory.0.join("grade.mjs");
    fs::write(&path, harness).unwrap();
    let output =
        Command::new(std::env::var_os("UBI_BENCHMARK_NODE").unwrap_or_else(|| "node".into()))
            .arg(path)
            .output()
            .expect("Node required: put node on PATH or set UBI_BENCHMARK_NODE");
    assert!(
        output.status.success(),
        "Node evaluator failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    print!("{}", String::from_utf8_lossy(&output.stdout));
}
