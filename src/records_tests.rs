//! Independent expectations derived from SPEC sections 3, 5, 7, 8, 9 and 14.
//! Expected results are constants from those contracts, not backend comparisons.
use crate::analyzer::{analyze, Analysis, ValueType};
use crate::interpreter::{invoke, InvocationError, Value};
use crate::source::{SourceFile, SourceSet};
use std::collections::BTreeMap;
use std::fs;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

fn analysis(items: &[(&str, &str)]) -> Analysis {
    let mut sources = SourceSet::default();
    for (id, text) in items {
        sources
            .insert(SourceFile::new(id, text.as_bytes().to_vec()).unwrap())
            .unwrap();
    }
    analyze(&sources)
}

fn checked(items: &[(&str, &str)]) -> Analysis {
    let analysis = analysis(items);
    assert!(
        analysis.diagnostics.is_empty(),
        "{:#?}",
        analysis.diagnostics
    );
    analysis
}

fn run(
    analysis: &Analysis,
    function: &str,
    arguments: Vec<Value>,
) -> Result<Value, InvocationError> {
    invoke(analysis, &("main.ubi".into(), function.into()), arguments)
}

fn node(analysis: &Analysis, assertions: &str) {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "ubi-records-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&path).unwrap();
    let output = crate::js_backend::emit(&crate::ir::lower(analysis).unwrap()).unwrap();
    for (id, source) in output {
        let file = path.join(id);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, source).unwrap();
    }
    fs::write(path.join("test.mjs"), format!("import assert from 'node:assert/strict';\nimport * as m from './main.mjs';\n{assertions}")).unwrap();
    let result = Command::new("node")
        .arg(path.join("test.mjs"))
        .output()
        .expect("Node required for record conformance");
    fs::remove_dir_all(&path).unwrap();
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn record_updates_preserve_aliases_and_structural_equality() {
    let source = r#"
record Pair { left: int, right: int }
record Box { pair: Pair }
record Empty {}
record FloatBox { value: float }
export fn aliases() -> int {
  let original = Pair { right: 2, left: 1 };
  let alias = original;
  let mut current = original;
  current = Pair { ...current, left: 7, };
  let nested = Box { pair: current };
  let changed = Box { ...nested, pair: Pair { ...nested.pair, right: 9 } };
  alias.left * 1000 + original.right * 100 + nested.pair.left * 10 + changed.pair.right
}
export fn equal() -> bool {
  Pair { right: 2, left: 1 } == Pair { left: 1, right: 2 }
    && Box { pair: Pair { left: 1, right: 2 } } != Box { pair: Pair { left: 1, right: 3 } }
    && Empty {} == Empty {}
}
export fn nan() -> bool { let a = FloatBox { value: 0.0 / 0.0 }; let alias = a; a == alias }
export fn signedZero() -> bool { FloatBox { value: -0.0 } == FloatBox { value: 0.0 } }
"#;
    let a = checked(&[("main.ubi", source)]);
    for (name, expected) in [
        ("aliases", Value::Int(1279)),
        ("equal", Value::Bool(true)),
        ("nan", Value::Bool(false)),
        ("signedZero", Value::Bool(true)),
    ] {
        assert_eq!(run(&a, name, vec![]), Ok(expected));
    }
    node(&a, "assert.equal(m.aliases(), 1279); assert.equal(m.equal(), true); assert.equal(m.nan(), false); assert.equal(m.signedZero(), true);");
}

#[test]
fn mutable_bindings_resolve_targets_before_rhs_and_preserve_early_returns() {
    let source = r#"
export fn scopes() -> int {
  let mut x = 2;
  let shadow = { let mut x = x + 3; x = x + 4; x };
  x = { let mut x = 10; x = x + 1; x };
  shadow * 100 + x
}
export fn rhsWrites() -> int { let mut x = 1; x = { x = 8; x + 2 }; x }
export fn returning(flag: bool) -> int {
  let mut x = 1;
  x = if (flag) { return 7; } else { 3 };
  x + 1
}
export fn recordReturn() -> int {
  let mut x = 1;
  x = { return 9; 1 / 0 };
  x
}
"#;
    let a = checked(&[("main.ubi", source)]);
    assert_eq!(run(&a, "scopes", vec![]), Ok(Value::Int(911)));
    assert_eq!(run(&a, "rhsWrites", vec![]), Ok(Value::Int(10)));
    assert_eq!(
        run(&a, "returning", vec![Value::Bool(true)]),
        Ok(Value::Int(7))
    );
    assert_eq!(
        run(&a, "returning", vec![Value::Bool(false)]),
        Ok(Value::Int(4))
    );
    assert_eq!(run(&a, "recordReturn", vec![]), Ok(Value::Int(9)));
    node(&a, "assert.equal(m.scopes(), 911); assert.equal(m.rhsWrites(), 10); assert.equal(m.returning(true), 7); assert.equal(m.returning(false), 4); assert.equal(m.recordReturn(), 9);");
}

#[test]
fn record_fields_and_update_base_evaluate_once_in_written_order() {
    let source = r#"
record P { a: int, b: int }
export fn written() -> int {
  let mut x = 0;
  let p = P { b: { x = x + 1; x }, a: { x = x + 1; x } };
  x * 100 + p.a * 10 + p.b
}
export fn update() -> int {
  let mut x = 0;
  let p = P { ...{ x = x + 1; P { a: x, b: x } }, b: { x = x + 1; x }, a: { x = x + 1; x } };
  x * 100 + p.a * 10 + p.b
}
export fn constructorFault() -> int { P { b: 1 / 0, a: 2147483647 + 1 }.a }
export fn updateFault() -> int { P { ...P { a: 1, b: 2 }, b: 1 / 0, a: 2147483647 + 1 }.a }
export fn baseFault() -> int { P { ...P { a: 2147483647 + 1, b: 0 }, b: 1 / 0 }.a }
export fn constructorReturn() -> int { P { b: { return 17; 0 }, a: 1 / 0 }.a }
export fn updateReturn() -> int { P { ...P { a: 0, b: 0 }, b: { return 19; 0 }, a: 1 / 0 }.a }
export fn baseReturn() -> int { P { ...{ return 23; P { a: 0, b: 0 } }, a: 1 / 0 }.a }
"#;
    let a = checked(&[("main.ubi", source)]);
    for (name, expected) in [
        ("written", 221),
        ("update", 332),
        ("constructorReturn", 17),
        ("updateReturn", 19),
        ("baseReturn", 23),
    ] {
        assert_eq!(run(&a, name, vec![]), Ok(Value::Int(expected)));
    }
    for (name, code) in [
        ("constructorFault", "UBI-R0002"),
        ("updateFault", "UBI-R0002"),
        ("baseFault", "UBI-R0001"),
    ] {
        let Err(InvocationError::Runtime(fault)) = run(&a, name, vec![]) else {
            panic!("expected fault for {name}")
        };
        assert_eq!(fault.code, code);
    }
    node(
        &a,
        r#"assert.equal(m.written(),221); assert.equal(m.update(),332); assert.equal(m.constructorReturn(),17); assert.equal(m.updateReturn(),19); assert.equal(m.baseReturn(),23);
for (const name of ['constructorFault','updateFault']) assert.throws(() => m[name](), e => e.code === 'UBI-R0002');
assert.throws(() => m.baseFault(), e => e.code === 'UBI-R0001');"#,
    );
}

#[test]
fn imported_records_keep_declaration_identity_across_module_paths() {
    let a = checked(&[
        ("lib/types.ubi", "export record P { value: int } export fn make() -> P { P { value: 8 } }"),
        ("lib/use.ubi", "import { P } from \"./types.ubi\"; export fn plus(p: P) -> P { P { ...p, value: p.value + 1 } }"),
        ("main.ubi", "import { P, make } from \"./lib/./types.ubi\"; import { plus } from \"./lib/use.ubi\"; export fn run() -> int { let p: P = plus(make()); p.value }"),
    ]);
    assert_eq!(run(&a, "run", vec![]), Ok(Value::Int(9)));
    node(&a, "assert.equal(m.run(), 9);");
}

#[test]
fn record_and_rebinding_errors_have_specified_codes_and_primary_spans() {
    let cases = [
        ("record P { x: int } fn f() { P {} }", "UBI0030", "P {}"),
        (
            "record P { x: int } fn f() { P { x: 1, extra: 2 } }",
            "UBI0030",
            "extra",
        ),
        (
            "record P { x: int } fn f() { P { x: 1, x: 2 } }",
            "UBI0030",
            "x",
        ),
        ("record P { x: int, x: int }", "UBI0030", "x"),
        (
            "record P { x: int } fn f() { P { x: 1 }.missing }",
            "UBI0030",
            "missing",
        ),
        ("fn f() { 1.missing }", "UBI0030", "missing"),
        (
            "record A { x: int } record B { x: int } fn f() { A { x: 1 } == B { x: 1 } }",
            "UBI0020",
            "B { x: 1 }",
        ),
        (
            "record A { x: int } record B { x: int } fn f() { A { ...B { x: 1 }, x: 2 } }",
            "UBI0020",
            "B { x: 1 }",
        ),
        (
            "record P { x: int } fn f() { P { x: true } }",
            "UBI0020",
            "true",
        ),
        (
            "record Hidden {} export fn f(p: Hidden) -> int { 1 }",
            "UBI0013",
            "Hidden",
        ),
        (
            "record Hidden {} export fn f() -> Hidden { Hidden {} }",
            "UBI0013",
            "Hidden",
        ),
        (
            "record Hidden {} export record Public { hidden: Hidden }",
            "UBI0013",
            "Hidden",
        ),
        ("fn f(p: int) { p = 2; }", "UBI0022", "p"),
        ("fn f() { let x = 1; x = 2; }", "UBI0022", "x"),
        ("fn f() { let mut x = 1; x = true; }", "UBI0020", "true"),
        ("fn f() { let mut x = 1; let mut x = 2; }", "UBI0011", "x"),
        ("fn f() { { let mut x = 1; }; x = 2; }", "UBI0010", "x"),
        (
            "record P { x: int } fn f() { let mut p = P { x: 1 }; p.x = 2; }",
            "UBI0003",
            "p.x",
        ),
    ];
    for (source, code, span_text) in cases {
        let a = analysis(&[("main.ubi", source)]);
        let found = a
            .diagnostics
            .iter()
            .find(|d| d.code == code && &source[d.primary.start..d.primary.end] == span_text);
        assert!(
            found.is_some(),
            "{source}: expected {code} on {span_text:?}, got {:#?}",
            a.diagnostics
        );
    }
}

#[test]
fn private_record_imports_and_same_shaped_distinct_modules_are_rejected() {
    let private = analysis(&[
        ("lib.ubi", "record P {}"),
        (
            "main.ubi",
            "import { P } from \"./lib.ubi\"; fn f() { P {} }",
        ),
    ]);
    assert!(private.diagnostics.iter().any(|d| d.code == "UBI0012"));
    let nominal = analysis(&[
        ("a.ubi", "export record P { x: int } export fn make() -> P { P { x: 1 } }"),
        ("b.ubi", "export record P { x: int } export fn accept(p: P) -> int { p.x }"),
        ("main.ubi", "import { make } from \"./a.ubi\"; import { accept } from \"./b.ubi\"; export fn run() -> int { accept(make()) }"),
    ]);
    assert!(nominal.diagnostics.iter().any(|d| d.code == "UBI0020"));
}

#[test]
fn record_syntax_and_duplicate_field_offsets_follow_the_slice_contract() {
    let source = "export fn run() -> int { let mut p: P = P { x: 1, }; p = P { ...p, x: 9, }; p.x } record P { x: int, }";
    let a = checked(&[("main.ubi", source)]);
    assert_eq!(run(&a, "run", vec![]), Ok(Value::Int(9)));
    node(&a, "assert.equal(m.run(),9);");
    for source in [
        "record P { x: int, x: int }",
        "record P { x: int } fn f() { P { x: 1, x: 2 } }",
        "record P { x: int } fn f() { P { ...P { x: 1 }, x: 2, x: 3 } }",
    ] {
        let a = analysis(&[("main.ubi", source)]);
        let expected = source.rfind("x:").unwrap();
        assert!(
            a.diagnostics.iter().any(|d| d.code == "UBI0030"
                && d.primary.start == expected
                && d.primary.end == expected + 1),
            "{source}: {:#?}",
            a.diagnostics
        );
    }
    for source in ["record Generic<T> { x: T }", "enum E { A }"] {
        assert!(
            analysis(&[("main.ubi", source)])
                .diagnostics
                .iter()
                .any(|d| d.code == "UBI0003"),
            "unexpectedly accepted deferred syntax: {source}"
        );
    }
    for source in [
        "record P { x: int } fn f(p: P) { P { ...p } }",
        "record P { x: int } fn f(p: P) { P { ...p x: 1 } }",
    ] {
        assert!(
            analysis(&[("main.ubi", source)])
                .diagnostics
                .iter()
                .any(|d| d.code == "UBI0002"),
            "accepted invalid update: {source}"
        );
    }
}

fn record_argument(a: &Analysis, function: &str, fields: BTreeMap<String, Value>) -> Value {
    let ValueType::Record(type_id) =
        a.signatures[&("main.ubi".into(), function.into())].parameters[0]
    else {
        panic!("record parameter expected")
    };
    Value::Record {
        type_id,
        fields: fields.into(),
    }
}

#[test]
fn host_records_decode_serialized_fields_and_reject_live_objects_unread() {
    let a = checked(&[(
        "main.ubi",
        r#"
export record Inner { value: int }
export record Input { inner: Inner, text: string, real: float, __proto__: int }
export fn sum(p: Input) -> int { p.inner.value + p.__proto__ }
export fn identity(p: Input) -> Input { p }
export fn realField(p: Input) -> float { p.real }
export fn constructed() -> Input { Input { __proto__: 6, inner: Inner { value: 3 }, text: "NaN", real: 2.0 } }
"#,
    )]);
    let inner_type = a.record_keys[&("main.ubi".into(), "Inner".into())];
    let fields = BTreeMap::from([
        (
            "inner".into(),
            Value::Record {
                type_id: inner_type,
                fields: BTreeMap::from([("value".into(), Value::Int(4))]).into(),
            },
        ),
        ("text".into(), Value::String("😀".into())),
        ("real".into(), Value::Float(1.5)),
        ("__proto__".into(), Value::Int(8)),
    ]);
    let value = record_argument(&a, "sum", fields.clone());
    assert_eq!(run(&a, "sum", vec![value]), Ok(Value::Int(12)));
    for invalid in [
        BTreeMap::new(),
        {
            let mut f = fields.clone();
            f.insert("extra".into(), Value::Int(0));
            f
        },
        {
            let mut f = fields.clone();
            f.insert("text".into(), Value::Int(0));
            f
        },
    ] {
        assert_eq!(
            run(&a, "sum", vec![record_argument(&a, "sum", invalid)]),
            Err(InvocationError::InvalidArguments)
        );
    }
    node(
        &a,
        r#"
const text = '{"inner":{"value":4},"text":"😀","real":1.5,"__proto__":8}';
assert.equal(m.sum(text),12);
const output = m.identity(text);
assert.equal(output.__proto__,8); assert.equal(output.inner.value,4);
assert.ok(Object.isFrozen(output)); assert.ok(Object.isFrozen(output.inner));
assert.equal(m.sum(JSON.stringify(output)),12);
const constructed=m.constructed(); assert.equal(constructed.__proto__,6); assert.equal(constructed.text,'NaN'); assert.equal(m.sum(JSON.stringify(constructed)),9);
let reads=0;
const live={get inner(){ reads++; throw Error('getter executed'); }};
assert.throws(()=>m.sum(live),TypeError); assert.equal(reads,0);
const proxy=new Proxy({}, {get(){reads++; throw Error('proxy executed');},ownKeys(){reads++; throw Error('proxy executed');},getOwnPropertyDescriptor(){reads++; throw Error('proxy executed');}});
assert.throws(()=>m.sum(proxy),TypeError); assert.equal(reads,0);
for(const bad of ['{}','{"inner":{"value":4},"text":"ok","real":1.5,"__proto__":8,"extra":1}','{"inner":{},"text":"ok","real":1.5,"__proto__":8}','{"inner":{"value":true},"text":"ok","real":1.5,"__proto__":8}','{"inner":{"value":2147483648},"text":"ok","real":1.5,"__proto__":8}','{"inner":{"value":4},"text":"\\ud800","real":1.5,"__proto__":8}','null','[]','{']) assert.throws(()=>m.sum(bad),TypeError);
for (const [tag, expected] of [['NaN',NaN],['+Infinity',Infinity],['-Infinity',-Infinity],['+0',0],['-0',-0]]) {
  const input=JSON.parse(text); input.real=tag;
  assert.ok(Object.is(m.realField(JSON.stringify(input)),expected));
}
"#,
    );
}

#[test]
fn host_unit_fields_use_json_null_and_decode_to_unit() {
    let a = checked(&[("main.ubi", "export record UnitBox { done: unit } export fn isUnit(p: UnitBox) -> bool { p.done == () } export fn echo(p: UnitBox) -> UnitBox { p }")]);
    let valid = record_argument(&a, "isUnit", BTreeMap::from([("done".into(), Value::Unit)]));
    assert_eq!(run(&a, "isUnit", vec![valid]), Ok(Value::Bool(true)));
    let invalid = record_argument(
        &a,
        "isUnit",
        BTreeMap::from([("done".into(), Value::String("null".into()))]),
    );
    assert_eq!(
        run(&a, "isUnit", vec![invalid]),
        Err(InvocationError::InvalidArguments)
    );
    node(
        &a,
        r#"
assert.equal(m.isUnit('{"done":null}'),true);
for (const bad of ['{}','{"done":"null"}','{"done":false}','{"done":0}','{"done":{}}']) assert.throws(()=>m.isUnit(bad),TypeError);
const output=m.echo('{"done":null}'); assert.ok(Object.isFrozen(output));
const serialized=JSON.stringify(output,(_key,value)=>value===undefined?null:value);
assert.equal(serialized,'{"done":null}'); assert.equal(m.isUnit(serialized),true);
"#,
    );
}

#[test]
fn record_depth_32_is_accepted_and_33_faults_or_rejects_before_execution() {
    // Distinct nested types avoid needing unsupported recursive/generic containers.
    let mut source = String::from("export record R0 { value: int }\n");
    for n in 1..=32 {
        source.push_str(&format!("export record R{n} {{ child: R{} }}\n", n - 1));
    }
    fn value(n: usize) -> String {
        let mut body = String::from("let v0 = R0 { value: 7 }; ");
        for level in 1..=n {
            body.push_str(&format!(
                "let v{level} = R{level} {{ child: v{} }}; ",
                level - 1
            ));
        }
        body.push_str(&format!("v{n}"));
        body
    }
    source.push_str(&format!("export fn atLimit() -> R31 {{ {} }}\nexport fn overLimit() -> R32 {{ {} }}\nexport fn host32(x: R31) -> int {{ 32 }}\nexport fn host33(x: R32) -> int {{ 1 / 0 }}", value(31), value(32)));
    let a = checked(&[("main.ubi", &source)]);
    assert!(matches!(
        run(&a, "atLimit", vec![]),
        Ok(Value::Record { .. })
    ));
    let Err(InvocationError::Runtime(fault)) = run(&a, "overLimit", vec![]) else {
        panic!("expected record depth fault")
    };
    assert_eq!(fault.code, "UBI-R0005");
    let mut fields = BTreeMap::from([("value".into(), Value::Int(7))]);
    let mut value = None;
    for n in 0..=32 {
        if let Some(child) = value {
            fields = BTreeMap::from([("child".into(), child)]);
        }
        let type_id = a.record_keys[&("main.ubi".into(), format!("R{n}"))];
        value = Some(Value::Record {
            type_id,
            fields: fields.clone().into(),
        });
        if n == 31 {
            assert_eq!(
                run(&a, "host32", vec![value.clone().unwrap()]),
                Ok(Value::Int(32))
            );
        }
    }
    assert_eq!(
        run(&a, "host33", vec![value.unwrap()]),
        Err(InvocationError::InvalidArguments)
    );
    node(
        &a,
        r#"
assert.ok(Object.isFrozen(m.atLimit()));
assert.throws(()=>m.overLimit(),e=>e.code==='UBI-R0005');
let value={value:7}; for(let i=0;i<31;i++) value={child:value};
assert.equal(m.host32(JSON.stringify(value)),32);
value={child:value}; assert.throws(()=>m.host33(JSON.stringify(value)),TypeError);
"#,
    );
}
