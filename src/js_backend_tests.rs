use crate::analyzer::analyze;
use crate::ir::lower;
use crate::js_backend::emit;
use crate::source::{SourceFile, SourceSet};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

fn generated(items: &[(&str, &[u8])]) -> std::collections::BTreeMap<String, String> {
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
    emit(&lower(&analysis).unwrap()).unwrap()
}

type RuntimeSource = (&'static str, &'static [u8]);
type RuntimeCase = (&'static str, Vec<RuntimeSource>);

fn runtime_cases() -> Vec<RuntimeCase> {
    let limits_source: &'static [u8] = b"export fn recurse(value: int) -> int { recurse(value + 1) }\nexport fn work(value: int) -> int { if (value <= 0) { 0 } else { work(value - 1) + work(value - 1) } }";
    let reserved_source: &'static [u8] = r#"export fn default() -> string { "quote: \"; slash: \\; snow: ☃" } export fn identity(value: string) -> string { value } export fn concat(left: string, right: string) -> string { left + right }"#.as_bytes();
    let escaped_import_main: &'static [u8] = b"import { default } from \"../lib/space name#part.ubi\"; export fn run() -> int { default() }";
    let escaped_import_lib: &'static [u8] = b"export fn default() -> int { 42 }";
    vec![
        (
            "arithmetic",
            vec![(
                "core/arithmetic.ubi",
                include_bytes!("../examples/core/arithmetic.ubi"),
            )],
        ),
        (
            "bindings",
            vec![(
                "core/bindings.ubi",
                include_bytes!("../examples/core/bindings.ubi"),
            )],
        ),
        (
            "control",
            vec![(
                "core/control.ubi",
                include_bytes!("../examples/core/control.ubi"),
            )],
        ),
        (
            "floats",
            vec![(
                "core/floats.ubi",
                include_bytes!("../examples/core/floats.ubi"),
            )],
        ),
        (
            "short-circuit",
            vec![(
                "core/short-circuit.ubi",
                include_bytes!("../examples/core/short-circuit.ubi"),
            )],
        ),
        (
            "imports",
            vec![
                (
                    "modules/app/main.ubi",
                    include_bytes!("../examples/modules/app/main.ubi"),
                ),
                (
                    "modules/lib/math.ubi",
                    include_bytes!("../examples/modules/lib/math.ubi"),
                ),
            ],
        ),
        (
            "escaped-import",
            vec![
                ("app/main.ubi", escaped_import_main),
                ("lib/space name#part.ubi", escaped_import_lib),
            ],
        ),
        (
            "overflow",
            vec![(
                "faults/overflow.ubi",
                include_bytes!("../examples/faults/overflow.ubi"),
            )],
        ),
        (
            "zero-divisor",
            vec![(
                "faults/zero-divisor.ubi",
                include_bytes!("../examples/faults/zero-divisor.ubi"),
            )],
        ),
        (
            "evaluation-order",
            vec![(
                "faults/evaluation-order.ubi",
                include_bytes!("../examples/faults/evaluation-order.ubi"),
            )],
        ),
        ("limits", vec![("main.ubi", limits_source)]),
        ("reserved", vec![("main.ubi", reserved_source)]),
    ]
}

#[test]
fn interpreter_matches_expected_results_for_the_node_runtime_corpus() {
    use crate::interpreter::{invoke, InvocationError, Value};
    use std::collections::BTreeMap;

    #[derive(Debug)]
    enum Expected {
        Int(i32),
        Float(f64),
        Bool(bool),
        String(&'static str),
        Unit,
        Fault(&'static str),
    }

    let mut analyses = BTreeMap::new();
    for (case, items) in runtime_cases() {
        let mut sources = SourceSet::default();
        for (id, bytes) in items {
            sources
                .insert(SourceFile::new(id, bytes.to_vec()).unwrap())
                .unwrap();
        }
        let analysis = analyze(&sources);
        assert!(
            analysis.diagnostics.is_empty(),
            "{case}: {:#?}",
            analysis.diagnostics
        );
        analyses.insert(case, analysis);
    }

    let calls = vec![
        (
            "arithmetic",
            "core/arithmetic.ubi",
            "evaluate",
            vec![],
            Expected::Int(-15),
        ),
        (
            "arithmetic",
            "core/arithmetic.ubi",
            "minimum",
            vec![],
            Expected::Int(i32::MIN),
        ),
        (
            "arithmetic",
            "core/arithmetic.ubi",
            "minimumRemainder",
            vec![],
            Expected::Int(0),
        ),
        (
            "bindings",
            "core/bindings.ubi",
            "shadow",
            vec![],
            Expected::Int(9),
        ),
        (
            "bindings",
            "core/bindings.ubi",
            "discard",
            vec![],
            Expected::Unit,
        ),
        (
            "bindings",
            "core/bindings.ubi",
            "unitEquality",
            vec![],
            Expected::Bool(true),
        ),
        (
            "control",
            "core/control.ubi",
            "factorial",
            vec![Value::Int(5)],
            Expected::Int(120),
        ),
        (
            "control",
            "core/control.ubi",
            "branch",
            vec![Value::Bool(true)],
            Expected::Int(7),
        ),
        (
            "control",
            "core/control.ubi",
            "branch",
            vec![Value::Bool(false)],
            Expected::Int(9),
        ),
        (
            "floats",
            "core/floats.ubi",
            "finite",
            vec![],
            Expected::Float(1.5),
        ),
        (
            "floats",
            "core/floats.ubi",
            "negativeZero",
            vec![],
            Expected::Float(-0.0),
        ),
        (
            "floats",
            "core/floats.ubi",
            "positiveInfinity",
            vec![],
            Expected::Float(f64::INFINITY),
        ),
        (
            "floats",
            "core/floats.ubi",
            "negativeInfinity",
            vec![],
            Expected::Float(f64::NEG_INFINITY),
        ),
        (
            "floats",
            "core/floats.ubi",
            "notANumber",
            vec![],
            Expected::Float(f64::NAN),
        ),
        (
            "floats",
            "core/floats.ubi",
            "rounding",
            vec![],
            Expected::Float(9_007_199_254_740_992.0),
        ),
        (
            "floats",
            "core/floats.ubi",
            "underflow",
            vec![],
            Expected::Float(0.0),
        ),
        (
            "floats",
            "core/floats.ubi",
            "comparisons",
            vec![],
            Expected::Bool(true),
        ),
        (
            "short-circuit",
            "core/short-circuit.ubi",
            "run",
            vec![],
            Expected::Bool(true),
        ),
        (
            "imports",
            "modules/app/main.ubi",
            "run",
            vec![],
            Expected::Int(42),
        ),
        (
            "escaped-import",
            "app/main.ubi",
            "run",
            vec![],
            Expected::Int(42),
        ),
        (
            "overflow",
            "faults/overflow.ubi",
            "addition",
            vec![],
            Expected::Fault("UBI-R0001"),
        ),
        (
            "overflow",
            "faults/overflow.ubi",
            "subtraction",
            vec![],
            Expected::Fault("UBI-R0001"),
        ),
        (
            "overflow",
            "faults/overflow.ubi",
            "multiplication",
            vec![],
            Expected::Fault("UBI-R0001"),
        ),
        (
            "overflow",
            "faults/overflow.ubi",
            "negation",
            vec![],
            Expected::Fault("UBI-R0001"),
        ),
        (
            "overflow",
            "faults/overflow.ubi",
            "division",
            vec![],
            Expected::Fault("UBI-R0001"),
        ),
        (
            "zero-divisor",
            "faults/zero-divisor.ubi",
            "division",
            vec![],
            Expected::Fault("UBI-R0002"),
        ),
        (
            "zero-divisor",
            "faults/zero-divisor.ubi",
            "remainder",
            vec![],
            Expected::Fault("UBI-R0002"),
        ),
        (
            "evaluation-order",
            "faults/evaluation-order.ubi",
            "arguments",
            vec![],
            Expected::Fault("UBI-R0002"),
        ),
        (
            "evaluation-order",
            "faults/evaluation-order.ubi",
            "operands",
            vec![],
            Expected::Fault("UBI-R0002"),
        ),
        (
            "evaluation-order",
            "faults/evaluation-order.ubi",
            "selectedBranch",
            vec![],
            Expected::Int(8),
        ),
        (
            "limits",
            "main.ubi",
            "recurse",
            vec![Value::Int(0)],
            Expected::Fault("UBI-R0005"),
        ),
        (
            "limits",
            "main.ubi",
            "work",
            vec![Value::Int(30)],
            Expected::Fault("UBI-R0005"),
        ),
        (
            "reserved",
            "main.ubi",
            "default",
            vec![],
            Expected::String("quote: \"; slash: \\; snow: ☃"),
        ),
        (
            "reserved",
            "main.ubi",
            "identity",
            vec![Value::String("😀".to_owned())],
            Expected::String("😀"),
        ),
        (
            "reserved",
            "main.ubi",
            "concat",
            vec![
                Value::String("snow".to_owned()),
                Value::String("☃".to_owned()),
            ],
            Expected::String("snow☃"),
        ),
    ];

    for (case, source_id, export, arguments, expected) in calls {
        let analysis = &analyses[case];
        let key = (source_id.to_owned(), export.to_owned());
        let actual = invoke(analysis, &key, arguments);
        match (actual, expected) {
            (Ok(Value::Int(actual)), Expected::Int(expected)) => {
                assert_eq!(actual, expected, "{case}::{export}")
            }
            (Ok(Value::Float(actual)), Expected::Float(expected)) if expected.is_nan() => {
                assert!(actual.is_nan(), "{case}::{export}")
            }
            (Ok(Value::Float(actual)), Expected::Float(expected)) => {
                assert_eq!(actual.to_bits(), expected.to_bits(), "{case}::{export}")
            }
            (Ok(Value::Bool(actual)), Expected::Bool(expected)) => {
                assert_eq!(actual, expected, "{case}::{export}")
            }
            (Ok(Value::String(actual)), Expected::String(expected)) => {
                assert_eq!(actual, expected, "{case}::{export}")
            }
            (Ok(Value::Unit), Expected::Unit) => {}
            (Err(InvocationError::Runtime(fault)), Expected::Fault(expected)) => {
                assert_eq!(fault.code, expected, "{case}::{export}")
            }
            (actual, expected) => panic!("{case}::{export}: expected {expected:?}, got {actual:?}"),
        }
    }
}

#[test]
fn emits_deterministic_checked_javascript_modules() {
    let items = [(
        "core/arithmetic.ubi",
        include_bytes!("../examples/core/arithmetic.ubi").as_slice(),
    )];
    let first = generated(&items);
    let second = generated(&items);
    assert_eq!(first, second);

    let js = &first["core/arithmetic.mjs"];
    assert!(js.contains("function $ubi_add"));
    assert!(js.contains("function $ubi_div"));
    assert!(js.contains("export { $ubi_export0 as evaluate"));
    assert!(js.contains("$ubi_enter($ubi_budget)"));
    assert!(js.contains("$ubi_leave($ubi_budget)"));
}

#[test]
fn imports_resolved_functions_with_relative_esm_specifiers() {
    let files = generated(&[
        (
            "modules/app/main.ubi",
            include_bytes!("../examples/modules/app/main.ubi"),
        ),
        (
            "modules/lib/math.ubi",
            include_bytes!("../examples/modules/lib/math.ubi"),
        ),
    ]);
    let app = &files["modules/app/main.mjs"];
    assert!(app.contains("import { double as $ubi_import0 } from \"../lib/math.mjs\";"));
    assert!(app.contains("$ubi_import0(($ubi_tick($ubi_budget), 21), $ubi_budget)"));
    assert!(app.contains("export { $ubi_export0 as run };"));
}

#[test]
fn escapes_strings_and_exports_reserved_javascript_names() {
    let files = generated(&[(
        "main.ubi",
        r#"export fn default() -> string { "quote: \"; slash: \\; snow: ☃" }"#.as_bytes(),
    )]);
    let js = &files["main.mjs"];
    assert!(js.contains("export { $ubi_export0 as default };"));
    assert!(js.contains("quote: \\\"; slash: \\\\; snow: ☃"));
}

#[test]
fn node_executes_the_complete_m1_runtime_corpus() {
    let cases = runtime_cases();

    let temp = TestDirectory::new();
    for (name, items) in cases {
        let output = generated(&items);
        let directory = temp.path.join(name);
        fs::create_dir_all(&directory).unwrap();
        for (path, source) in output {
            let destination = directory.join(path);
            fs::create_dir_all(destination.parent().unwrap()).unwrap();
            fs::write(destination, source).unwrap();
        }
    }

    let harness = r#"import assert from "node:assert/strict";
import * as arithmetic from "./arithmetic/core/arithmetic.mjs";
import * as bindings from "./bindings/core/bindings.mjs";
import * as control from "./control/core/control.mjs";
import * as floats from "./floats/core/floats.mjs";
import * as shortCircuit from "./short-circuit/core/short-circuit.mjs";
import * as imported from "./imports/modules/app/main.mjs";
import * as escapedImport from "./escaped-import/app/main.mjs";
import * as overflow from "./overflow/faults/overflow.mjs";
import * as zero from "./zero-divisor/faults/zero-divisor.mjs";
import * as order from "./evaluation-order/faults/evaluation-order.mjs";
import * as limits from "./limits/main.mjs";
import reservedText, * as reserved from "./reserved/main.mjs";

function fault(fn, code) {
  assert.throws(fn, error => error.code === code && error.message.length > 0);
}

assert.equal(arithmetic.evaluate(), -15);
assert.equal(arithmetic.minimum(), -2147483648);
assert.equal(arithmetic.minimumRemainder(), 0);
assert.equal(bindings.shadow(), 9);
assert.equal(bindings.discard(), undefined);
assert.equal(bindings.unitEquality(), true);
assert.equal(control.factorial(5), 120);
assert.equal(control.branch(true), 7);
assert.equal(control.branch(false), 9);
assert.equal(floats.finite(), 1.5);
assert.ok(Object.is(floats.negativeZero(), -0));
assert.equal(floats.positiveInfinity(), Infinity);
assert.equal(floats.negativeInfinity(), -Infinity);
assert.ok(Number.isNaN(floats.notANumber()));
assert.equal(floats.rounding(), 9007199254740992);
assert.ok(Object.is(floats.underflow(), 0));
assert.equal(floats.comparisons(), true);
assert.equal(shortCircuit.run(), true);
assert.equal(imported.run(), 42);
assert.equal(escapedImport.run(), 42);
for (const name of ["addition", "subtraction", "multiplication", "negation", "division"]) fault(overflow[name], "UBI-R0001");
for (const name of ["division", "remainder"]) fault(zero[name], "UBI-R0002");
fault(order.arguments, "UBI-R0002");
fault(order.operands, "UBI-R0002");
assert.equal(order.selectedBranch(), 8);
fault(() => limits.recurse(0), "UBI-R0005");
fault(() => limits.work(30), "UBI-R0005");
assert.equal(reservedText(), 'quote: "; slash: \\; snow: ☃');
assert.equal(reserved.identity("😀"), "😀");
assert.equal(reserved.concat("snow", "☃"), "snow☃");
assert.throws(() => reserved.identity("\ud800"), TypeError);
assert.throws(() => control.branch(1), TypeError);
"#;
    fs::write(temp.path.join("conformance.mjs"), harness).unwrap();
    let output = Command::new("node")
        .arg(temp.path.join("conformance.mjs"))
        .output()
        .expect("Node.js must be available for backend conformance tests");
    assert!(
        output.status.success(),
        "Node conformance failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "ubi-js-conformance-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
