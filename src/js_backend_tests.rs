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

#[test]
fn emits_deterministic_checked_javascript_modules() {
    let items = [(
        "core/arithmetic.ubi",
        include_bytes!("../examples/core/arithmetic.ubi").as_slice(),
    )];
    let first = generated(&items);
    let second = generated(&items);
    assert_eq!(first, second);

    let js = &first["core/arithmetic.js"];
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
    let app = &files["modules/app/main.js"];
    assert!(app.contains("import { double as $ubi_import0 } from \"../lib/math.js\";"));
    assert!(app.contains("$ubi_import0(($ubi_tick($ubi_budget), 21), $ubi_budget)"));
    assert!(app.contains("export { $ubi_export0 as run };"));
}

#[test]
fn escapes_strings_and_exports_reserved_javascript_names() {
    let files = generated(&[(
        "main.ubi",
        r#"export fn default() -> string { "quote: \"; slash: \\; snow: ☃" }"#.as_bytes(),
    )]);
    let js = &files["main.js"];
    assert!(js.contains("export { $ubi_export0 as default };"));
    assert!(js.contains("quote: \\\"; slash: \\\\; snow: ☃"));
}

#[test]
fn node_executes_the_complete_m1_runtime_corpus() {
    let limits_source: &[u8] = b"export fn recurse(value: int) -> int { recurse(value + 1) }\nexport fn work(value: int) -> int { if (value <= 0) { 0 } else { work(value - 1) + work(value - 1) } }";
    let reserved_source: &[u8] = r#"export fn default() -> string { "quote: \"; slash: \\; snow: ☃" } export fn identity(value: string) -> string { value } export fn concat(left: string, right: string) -> string { left + right }"#.as_bytes();
    let escaped_import_main: &[u8] = b"import { default } from \"../lib/space name#part.ubi\"; export fn run() -> int { default() }";
    let escaped_import_lib: &[u8] = b"export fn default() -> int { 42 }";
    let cases: Vec<(&str, Vec<(&str, &[u8])>)> = vec![
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
    ];

    let temp = TestDirectory::new();
    for (name, items) in cases {
        let output = generated(&items);
        let directory = temp.path.join(name);
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("package.json"), "{\"type\":\"module\"}").unwrap();
        for (path, source) in output {
            let destination = directory.join(path);
            fs::create_dir_all(destination.parent().unwrap()).unwrap();
            fs::write(destination, source).unwrap();
        }
    }

    let harness = r#"import assert from "node:assert/strict";
import * as arithmetic from "./arithmetic/core/arithmetic.js";
import * as bindings from "./bindings/core/bindings.js";
import * as control from "./control/core/control.js";
import * as floats from "./floats/core/floats.js";
import * as shortCircuit from "./short-circuit/core/short-circuit.js";
import * as imported from "./imports/modules/app/main.js";
import * as escapedImport from "./escaped-import/app/main.js";
import * as overflow from "./overflow/faults/overflow.js";
import * as zero from "./zero-divisor/faults/zero-divisor.js";
import * as order from "./evaluation-order/faults/evaluation-order.js";
import * as limits from "./limits/main.js";
import reservedText, * as reserved from "./reserved/main.js";

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
