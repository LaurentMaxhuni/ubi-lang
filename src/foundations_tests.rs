//! Independent expectations from SPEC section 15; both backends meet fixed results.
use crate::analyzer::{analyze, Analysis, TypeInfo, ValueType};
use crate::interpreter::{invoke, InvocationError, Value};
use crate::source::{SourceFile, SourceSet};
use std::fs;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

fn analysis(source: &str) -> Analysis {
    let mut sources = SourceSet::default();
    sources
        .insert(SourceFile::new("main.ubi", source.as_bytes().to_vec()).unwrap())
        .unwrap();
    analyze(&sources)
}

fn checked(source: &str) -> Analysis {
    let a = analysis(source);
    assert!(a.diagnostics.is_empty(), "{:#?}", a.diagnostics);
    a
}

fn run(a: &Analysis, name: &str, args: Vec<Value>) -> Result<Value, InvocationError> {
    invoke(a, &("main.ubi".into(), name.into()), args)
}

fn fault(a: &Analysis, name: &str, code: &str) {
    let Err(InvocationError::Runtime(fault)) = run(a, name, vec![]) else {
        panic!("expected {code} from {name}");
    };
    assert_eq!(fault.code, code, "{name}");
}

fn node(a: &Analysis, assertions: &str) {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "ubi-foundations-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&path).unwrap();
    for (id, source) in crate::js_backend::emit(&crate::ir::lower(a).unwrap()).unwrap() {
        let file = path.join(id);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, source).unwrap();
    }
    fs::write(path.join("test.mjs"), format!("import assert from 'node:assert/strict';\nimport * as m from './main.mjs';\n{assertions}")).unwrap();
    let result = Command::new("node")
        .arg(path.join("test.mjs"))
        .output()
        .expect("Node required for foundations conformance");
    fs::remove_dir_all(&path).unwrap();
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn lists_options_and_immutable_updates_have_structural_values() {
    let a = checked(
        r#"
fn count(xs: List<int>) -> int { length(xs) }
fn empty() -> List<int> { [] }
export fn values() -> int {
  let original = [2, 4]; let alias = original;
  let longer = append(original, 6);
  let changed = match (set(longer, 1, 9)) { Option.Some(xs) => xs, Option.None => [] };
  let selected = match (changed[1]) { Option.Some(x) => x, Option.None => 0 };
  length(alias) * 100 + length(longer) * 10 + selected
}
export fn emptyContexts() -> int {
  let xs: List<Option<List<int>>> = [Option.None, Option.Some([])];
  let no: Option<int> = Option.None;
  count([]) + length(empty()) + length(xs) + match (no) { Option.Some(x) => x, Option.None => 5 }
}
export fn bounds() -> bool {
  let xs = [1]; let none: Option<int> = Option.None;
  let noList: Option<List<int>> = Option.None;
  xs[-1] == none && xs[1] == none && set(xs, -1, 7) == noList && set(xs, 1, 7) == noList
    && xs == [1] && append(xs, 2) == [1, 2]
}
export fn equality() -> bool {
  let a: Option<Option<int>> = Option.Some(Option.None);
  let b: Option<Option<int>> = Option.Some(Option.None);
  a == b && [[1, 2], []] == [[1, 2], []] && [0.0] == [-0.0]
}
export fn nanEquality() -> bool { let a = [0.0 / 0.0]; let alias = a; a == alias }
export fn nestedMatch() -> int {
  let x: Option<Option<bool>> = Option.Some(Option.Some(false));
  match (x) {
    Option.None => 1,
    Option.Some(Option.None) => 2,
    Option.Some(Option.Some(true)) => 3,
    Option.Some(Option.Some(false)) => 4,
  }
}
"#,
    );
    for (name, expected) in [
        ("values", Value::Int(239)),
        ("emptyContexts", Value::Int(7)),
        ("bounds", Value::Bool(true)),
        ("equality", Value::Bool(true)),
        ("nanEquality", Value::Bool(false)),
        ("nestedMatch", Value::Int(4)),
    ] {
        assert_eq!(run(&a, name, vec![]), Ok(expected), "{name}");
    }
    node(&a, "assert.equal(m.values(),239); assert.equal(m.emptyContexts(),7); assert.equal(m.bounds(),true); assert.equal(m.equality(),true); assert.equal(m.nanEquality(),false); assert.equal(m.nestedMatch(),4);");
}

#[test]
fn expressions_and_match_guards_evaluate_once_in_source_order() {
    let a = checked(
        r#"
export fn order() -> int {
  let mut n = 0;
  let xs = [{ n = n + 1; n }, { n = n + 1; n }];
  let result = match ({ n = n + 1; Option.Some(7) }) {
    Option.Some(x) if ({ n = n + 1; false }) => 1 / 0,
    Option.Some(x) if ({ n = n + 1; true }) => x,
    Option.Some(x) => 1 / 0, Option.None => 1 / 0,
  };
  n * 100 + match (xs[0]) { Option.Some(x) => x * 10, Option.None => 0 } + result
}
export fn listReturn() -> int { let xs = [{ return 17; 0 }, 1 / 0]; length(xs) }
export fn listFault() -> int { length([1 / 0, 2147483647 + 1]) }
export fn setFault() -> Option<List<int>> { set([1], -1, 1 / 0) }
export fn guardReturn() -> int {
  match (Option.Some(1)) { Option.Some(x) if ({ return 19; true }) => x, _ => 1 / 0 }
}
export fn literalPatterns() -> int {
  match (-3) { -3 => 4, _ => 0 } + match ("😀") { "😀" => 5, _ => 0 }
}
"#,
    );
    for (name, expected) in [
        ("order", 517),
        ("listReturn", 17),
        ("guardReturn", 19),
        ("literalPatterns", 9),
    ] {
        assert_eq!(run(&a, name, vec![]), Ok(Value::Int(expected)));
    }
    fault(&a, "listFault", "UBI-R0002");
    fault(&a, "setFault", "UBI-R0002");
    node(&a, "assert.equal(m.order(),517); assert.equal(m.listReturn(),17); assert.equal(m.guardReturn(),19); assert.equal(m.literalPatterns(),9); for(const name of ['listFault','setFault']) assert.throws(()=>m[name](),e=>e.code==='UBI-R0002');");
}

#[test]
fn loops_use_snapshot_iterables_fresh_scopes_and_innermost_control() {
    let a = checked(
        r#"
export fn snapshot() -> int {
  let mut xs = [1, 2, 3]; let mut evaluations = 0; let mut sum = 0;
  for (x in { evaluations = evaluations + 1; xs }) {
    let local = x; xs = append(xs, 99); sum = sum + local;
  };
  evaluations * 100 + length(xs) * 10 + sum
}
export fn controls() -> int {
  let mut total = 0;
  for (x in range(0, 4)) {
    if (x == 1) { continue; }
    for (y in range(0, 4)) {
      if (y == 2) { break; }
      total = total + x * 10 + y;
    }
  }
  let mut n = 0;
  while (n < 4) { n = n + 1; if (n == 2) { continue; } total = total + n; }
  total
}
export fn rangeEdges() -> int {
  let mut count = 0;
  for (x in range(2147483646, 2147483647)) { count = count + 1; }
  for (x in range(-2147483648, -2147483647)) { count = count + 1; }
  for (x in range(9, 9)) { return 1 / 0; }
  for (x in range(9, -9)) { return 1 / 0; }
  count
}
export fn scalarLoop() -> string { let mut out = ""; for (c in "a😀é") { out = out + c + "."; } out }
export fn loopReturn() -> int { while (true) { for (x in [1]) { return x + 8; } } 0 }
export fn infiniteWhile() -> unit { while (true) {} }
export fn infiniteContinue() -> unit { while (true) { continue; } }
export fn hugeRange() -> unit { for (x in range(-2147483648, 2147483647)) {} }
"#,
    );
    for (name, expected) in [
        ("snapshot", 166),
        ("controls", 111),
        ("rangeEdges", 2),
        ("loopReturn", 9),
    ] {
        assert_eq!(run(&a, name, vec![]), Ok(Value::Int(expected)), "{name}");
    }
    assert_eq!(
        run(&a, "scalarLoop", vec![]),
        Ok(Value::String("a.😀.é.".into()))
    );
    for name in ["infiniteWhile", "infiniteContinue", "hugeRange"] {
        fault(&a, name, "UBI-R0005");
    }
    node(&a, "assert.equal(m.snapshot(),166); assert.equal(m.controls(),111); assert.equal(m.rangeEdges(),2); assert.equal(m.loopReturn(),9); assert.equal(m.scalarLoop(),'a.😀.é.'); for(const name of ['infiniteWhile','infiniteContinue','hugeRange']) assert.throws(()=>m[name](),e=>e.code==='UBI-R0005');");
}

#[test]
fn closures_capture_snapshots_and_returns_stay_inside_callbacks() {
    let a = checked(
        r#"
fn twice(x: int) -> int { x * 2 }
export fn capture() -> int {
  let mut n = 3; let before = (x: int) => x + n;
  n = 10; let after = (x: int) => { let mut n = n + x; n = n + 1; n };
  let xs = [1, 2]; let saved = (x: int) => length(xs) + x;
  before(2) * 100 + after(2) * 10 + saved(4)
}
export fn helpers() -> int {
  let ys = map([1, 2, 3, 4], twice);
  let selected = filter(ys, (x: int) => x > 4);
  let first = match (find(selected, (x: int) => x > 6)) { Option.Some(x) => x, Option.None => 0 };
  fold(selected, 1, (acc: int, x: int) => acc * 10 + x) + first
}
export fn helperResultTypes() -> string {
  let mapped = map([1, 2], (n: int) => toString(n));
  join(mapped, ",") + ":" + fold([3, 4], "", (text: string, n: int) => text + toString(n))
}
export fn localReturns() -> int {
  let f = (x: int) => { if (x == 1) { return 7; } x + 1 };
  fold(map([1, 2], f), 0, (a: int, b: int) => a * 10 + b) + 100
}
export fn emptyCallbacks() -> int {
  let xs: List<int> = [];
  let ys = map(xs, (x: int) => 1 / 0);
  let zs = filter(xs, (x: int) => { let y = 1 / 0; y == 0 });
  let no: Option<int> = Option.None;
  if (find(xs, (x: int) => { let y = 1 / 0; y == 0 }) == no) {
    fold(xs, 9, (a: int, b: int) => 1 / 0) + length(ys) + length(zs)
  } else { 0 }
}
export fn callbackSetupOrder() -> int {
  let mut n = 0;
  let xs = map({ n = n + 1; [n] }, { n = n + 1; (x: int) => x * 10 + n });
  n * 100 + match (xs[0]) { Option.Some(x) => x, Option.None => 0 }
}
export fn findStops() -> int {
  match (find([1, 0], (x: int) => 1 / x == 1)) { Option.Some(x) => x, Option.None => 0 }
}
export fn mapFaultOrder() -> List<int> { map([0, 2147483647], (x: int) => if (x == 0) { 1 / 0 } else { x + 1 }) }
export fn filterFaultOrder() -> List<int> { filter([0, 2147483647], (x: int) => if (x == 0) { 1 / 0 == 0 } else { x + 1 == 0 }) }
export fn foldFaultOrder() -> int { fold([0, 2147483647], 0, (a: int, x: int) => if (x == 0) { 1 / 0 } else { x + 1 }) }
fn recurse(n: int) -> int { if (n == 0) { 0 } else { recurse(n - 1) } }
export fn callbackFrames() -> List<int> { map([40], recurse) }
"#,
    );
    for (name, expected) in [
        ("capture", 636),
        ("helpers", 176),
        ("localReturns", 173),
        ("emptyCallbacks", 9),
        ("callbackSetupOrder", 212),
        ("findStops", 1),
    ] {
        assert_eq!(run(&a, name, vec![]), Ok(Value::Int(expected)), "{name}");
    }
    for name in ["mapFaultOrder", "filterFaultOrder", "foldFaultOrder"] {
        fault(&a, name, "UBI-R0002");
    }
    fault(&a, "callbackFrames", "UBI-R0005");
    assert_eq!(
        run(&a, "helperResultTypes", vec![]),
        Ok(Value::String("1,2:34".into()))
    );
    node(&a, "assert.equal(m.capture(),636); assert.equal(m.helpers(),176); assert.equal(m.helperResultTypes(),'1,2:34'); assert.equal(m.localReturns(),173); assert.equal(m.emptyCallbacks(),9); assert.equal(m.callbackSetupOrder(),212); assert.equal(m.findStops(),1); for(const name of ['mapFaultOrder','filterFaultOrder','foldFaultOrder']) assert.throws(()=>m[name](),e=>e.code==='UBI-R0002'); assert.throws(()=>m.callbackFrames(),e=>e.code==='UBI-R0005');");
}

#[test]
fn unicode_strings_use_scalars_and_literal_nonoverlapping_operations() {
    let a = checked(
        r#"
export fn text() -> string {
  let s = "a😀e\u{301}";
  let second = match (s[1]) { Option.Some(c) => c, Option.None => "bad" };
  second + ":" + slice(s, 1, 3) + ":" + slice(s, -8, 1) + ":" + trim(" \t\r\nhi\n ")
}
export fn scalarLength() -> int { length("a😀e\u{301}") }
export fn predicates() -> bool {
  contains("a😀b", "😀") && startsWith("a😀b", "a😀") && endsWith("a😀b", "😀b")
    && contains("", "") && startsWith("", "") && endsWith("", "")
    && trim("\u{a0}x\u{a0}") == "\u{a0}x\u{a0}" && slice("abc", 2, 1) == ""
    && slice("abc", 9, 99) == "" && slice("abc", 0, 99) == "abc"
}
export fn splitting() -> bool {
  let empty: List<string> = [];
  split("::a::::", "::") == ["", "a", "", ""] && split("aaa", "aa") == ["", "a"]
    && split("😀é", "") == ["😀", "é"] && split("", "") == empty && split("", ",") == [""]
    && join(["", "a", ""], "|") == "|a|"
}
export fn replacing() -> string { replace("😀a", "", "_") + ":" + replace("aaaaa", "aa", "x") + ":" + replace("", "", "!") }
export fn conversions() -> string { toString(-2147483648) + ":" + toString(true) + ":" + toString("😀") }
"#,
    );
    assert_eq!(
        run(&a, "text", vec![]),
        Ok(Value::String("😀:😀e:a:hi".into()))
    );
    assert_eq!(run(&a, "scalarLength", vec![]), Ok(Value::Int(4)));
    for name in ["predicates", "splitting"] {
        assert_eq!(run(&a, name, vec![]), Ok(Value::Bool(true)));
    }
    assert_eq!(
        run(&a, "replacing", vec![]),
        Ok(Value::String("_😀_a_:xxa:!".into()))
    );
    assert_eq!(
        run(&a, "conversions", vec![]),
        Ok(Value::String("-2147483648:true:😀".into()))
    );
    node(&a, "assert.equal(m.text(),'😀:😀e:a:hi'); assert.equal(m.scalarLength(),4); assert.equal(m.predicates(),true); assert.equal(m.splitting(),true); assert.equal(m.replacing(),'_😀_a_:xxa:!'); assert.equal(m.conversions(),'-2147483648:true:😀');");
}

#[test]
fn strict_numeric_parsing_and_checked_conversions_reject_partial_inputs() {
    let a = checked(
        r#"
export fn integer(text: string) -> int { match (parseInt(text)) { Option.Some(x) => x, Option.None => 99 } }
export fn real(text: string) -> float { match (parseFloat(text)) { Option.Some(x) => x, Option.None => 99.0 } }
export fn convert(x: float) -> int { match (toInt(x)) { Option.Some(n) => n, Option.None => 99 } }
export fn widening() -> float { toFloat(2147483647) }
"#,
    );
    for (text, expected) in [
        ("+0012", 12),
        ("-2147483648", i32::MIN),
        ("2147483647", i32::MAX),
        ("-0", 0),
        ("", 99),
        ("+", 99),
        (" 1", 99),
        ("1 ", 99),
        ("1x", 99),
        ("0x10", 99),
        ("1_0", 99),
        ("2147483648", 99),
        ("-2147483649", 99),
        ("١", 99),
    ] {
        assert_eq!(
            run(&a, "integer", vec![Value::String(text.into())]),
            Ok(Value::Int(expected)),
            "{text:?}"
        );
    }
    for (text, expected) in [
        ("1", 1.0),
        ("+01.25e+2", 125.0),
        ("-2E-1", -0.2),
        ("1e-9999", 0.0),
        (".5", 99.0),
        ("1.", 99.0),
        ("1e", 99.0),
        ("NaN", 99.0),
        ("Infinity", 99.0),
        ("1e9999", 99.0),
        (" 1", 99.0),
        ("1x", 99.0),
        ("1\n", 99.0),
    ] {
        assert_eq!(
            run(&a, "real", vec![Value::String(text.into())]),
            Ok(Value::Float(expected)),
            "{text:?}"
        );
    }
    for (x, expected) in [
        (2.9, 2),
        (-2.9, -2),
        (2147483647.9, i32::MAX),
        (-2147483648.9, i32::MIN),
        (2147483648.0, 99),
        (-2147483649.0, 99),
        (f64::INFINITY, 99),
        (f64::NAN, 99),
    ] {
        assert_eq!(
            run(&a, "convert", vec![Value::Float(x)]),
            Ok(Value::Int(expected))
        );
    }
    assert_eq!(run(&a, "widening", vec![]), Ok(Value::Float(2147483647.0)));
    let Ok(Value::Float(z)) = run(&a, "real", vec![Value::String("-0.0".into())]) else {
        panic!("float expected")
    };
    assert!(z == 0.0 && z.is_sign_negative());
    node(
        &a,
        r#"
for(const [s,n] of [['+0012',12],['-2147483648',-2147483648],['2147483647',2147483647],['-0',0]]) assert.equal(m.integer(s),n);
for(const s of ['', '+',' 1','1 ','1x','0x10','1_0','2147483648','-2147483649','١']) assert.equal(m.integer(s),99);
for(const [s,n] of [['1',1],['+01.25e+2',125],['-2E-1',-0.2],['1e-9999',0]]) assert.equal(m.real(s),n);
for(const s of ['.5','1.','1e','NaN','Infinity','1e9999',' 1','1x','1\n']) assert.equal(m.real(s),99);
assert.ok(Object.is(m.real('-0.0'),-0));
for(const [x,n] of [[2.9,2],[-2.9,-2],[2147483647.9,2147483647],[-2147483648.9,-2147483648],[2147483648,99],[-2147483649,99],[Infinity,99],[NaN,99]]) assert.equal(m.convert(x),n);
assert.equal(m.widening(),2147483647);
"#,
    );
}

#[test]
fn math_obeys_integer_faults_ieee_domains_and_signed_zero() {
    let a = checked(
        r#"
export fn integers() -> int { abs(-3) + min(7, 2) + max(1, 4) + clamp(9, 1, 5) }
export fn rounding() -> float { floor(1.9) + ceil(-1.9) + round(2.5) + round(-2.5) }
export fn root() -> float { sqrt(9.0) + pow(2.0, 3.0) + sin(0.0) + cos(0.0) + tan(0.0) + log(exp(1.0)) }
export fn minZero() -> float { min(0.0, -0.0) }
export fn maxZero() -> float { max(-0.0, 0.0) }
export fn roundZero() -> float { round(-0.2) }
export fn absZero() -> float { abs(-0.0) }
export fn sqrtNegative() -> float { sqrt(-1.0) }
export fn logZero() -> float { log(0.0) }
export fn nanMin() -> float { min(1.0, 0.0 / 0.0) }
export fn nanMax() -> float { max(0.0 / 0.0, 1.0) }
export fn nanClamp() -> float { clamp(1.0, 0.0 / 0.0, 2.0) }
export fn integerAbsFault() -> int { abs(-2147483648) }
export fn clampFault() -> float { clamp(1.0, 3.0, 2.0) }
"#,
    );
    assert_eq!(run(&a, "integers", vec![]), Ok(Value::Int(14)));
    assert_eq!(run(&a, "rounding", vec![]), Ok(Value::Float(0.0)));
    let Ok(Value::Float(root)) = run(&a, "root", vec![]) else {
        panic!("float expected")
    };
    assert!((root - 13.0).abs() < 1e-12);
    for (name, negative) in [
        ("minZero", true),
        ("maxZero", false),
        ("roundZero", true),
        ("absZero", false),
    ] {
        let Ok(Value::Float(z)) = run(&a, name, vec![]) else {
            panic!("float expected")
        };
        assert!(z == 0.0 && z.is_sign_negative() == negative, "{name}: {z}");
    }
    for name in ["sqrtNegative", "nanMin", "nanMax", "nanClamp"] {
        assert!(
            matches!(run(&a, name, vec![]), Ok(Value::Float(x)) if x.is_nan()),
            "{name}"
        );
    }
    assert_eq!(
        run(&a, "logZero", vec![]),
        Ok(Value::Float(f64::NEG_INFINITY))
    );
    fault(&a, "integerAbsFault", "UBI-R0001");
    fault(&a, "clampFault", "UBI-R0003");
    node(&a, "assert.equal(m.integers(),14); assert.equal(m.rounding(),0); assert.ok(Math.abs(m.root()-13)<1e-12); assert.ok(Object.is(m.minZero(),-0)); assert.ok(Object.is(m.maxZero(),0)); assert.ok(Object.is(m.roundZero(),-0)); assert.ok(Object.is(m.absZero(),0)); for(const name of ['sqrtNegative','nanMin','nanMax','nanClamp']) assert.ok(Number.isNaN(m[name]())); assert.equal(m.logZero(),-Infinity); assert.throws(()=>m.integerAbsFault(),e=>e.code==='UBI-R0001'); assert.throws(()=>m.clampFault(),e=>e.code==='UBI-R0003');");
}

#[test]
fn invalid_foundation_types_coverage_callbacks_and_loop_placement_are_rejected() {
    let cases = [
        "fn f() { let xs = []; }",
        "fn f() { let x = Option.None; }",
        "fn f() { let xs = [1, true]; }",
        "fn f(xs: List<int, bool>) {}",
        "fn f(x: Option) {}",
        "fn f() { [1][true]; }",
        "fn f() { match (Option.Some(true)) { Option.None => 0, Option.Some(true) => 1 }; }",
        "fn f() { match (Option.Some(1)) { Option.None => 0, Option.Some(x) if (true) => x }; }",
        "fn f() { match (true) { true => 0 }; }",
        "fn f() { match (1) { 1 => 0 }; }",
        "fn f() { match (true) { true => 1, false => false }; }",
        "fn f() { match (true) { true if (1) => 1, _ => 0 }; }",
        "fn f() { match (1) { Option.None => 0, _ => 0 }; }",
        "fn f() { break; }",
        "fn f() { continue; }",
        "fn f() { while (1) {} }",
        "fn f() { for (x in 1) {} }",
        "fn f() { for (x in [1]) { x } }",
        "fn f() { while (true) { let c = (x: int) => { break; }; } }",
        "fn f() { while (true) { let c = (x: int) => { continue; }; } }",
        "fn f() { let xs = [(x: int) => x]; }",
        "fn f() { let x = Option.Some((x: int) => x); }",
        "fn f() { let c = (x: int) => x; c == c; }",
        "fn f() { map([1], (x: bool) => x); }",
        "fn f() { filter([1], (x: int) => x); }",
        "fn f() { fold([1], 0, (a: int, x: int) => true); }",
        "fn f() { let c = (x: int) => x; c(true); }",
        "fn f() { min(1, 2.0); }",
        "fn f() { sqrt(1); }",
        "fn f() { toString(1.5); }",
        "fn f() { if (true) { 1 }; }",
    ];
    for source in cases {
        let a = analysis(source);
        assert!(
            a.diagnostics.iter().any(|d| d.code == "UBI0020"),
            "{source}: {:#?}",
            a.diagnostics
        );
        assert!(
            crate::ir::lower(&a).is_err(),
            "invalid program lowered: {source}"
        );
    }
    for source in [
        "fn f() { for (x in [1]) { x = 2; } }",
        "fn f() { let mut n = 1; let c = (x: int) => { n = 2; }; }",
    ] {
        assert!(
            analysis(source)
                .diagnostics
                .iter()
                .any(|d| d.code == "UBI0022"),
            "{source}"
        );
    }
    for source in [
        "fn f() { match (1) { x => x }; x; }",
        "fn f() { for (x in [1]) {} x; }",
        "fn f() { let c = (x: int) => missing; }",
    ] {
        assert!(
            analysis(source)
                .diagnostics
                .iter()
                .any(|d| d.code == "UBI0010"),
            "{source}"
        );
    }
}

#[test]
fn aggregate_host_validation_and_recursive_freezing_are_strict() {
    let a = checked(
        r#"
export record Item { value: int }
export fn identity(xs: List<Option<Item>>) -> List<Option<Item>> { xs }
export fn nested(x: Option<Option<int>>) -> Option<Option<int>> { x }
export fn total(xs: List<Option<Item>>) -> int {
  fold(xs, 0, (sum: int, item: Option<Item>) => sum + match (item) { Option.Some(x) => x.value, Option.None => 0 })
}
"#,
    );
    let ValueType::List(list_id) = a.signatures[&("main.ubi".into(), "total".into())].parameters[0]
    else {
        panic!("list expected")
    };
    let TypeInfo::List(ValueType::Option(option_id)) = a.types[list_id] else {
        panic!("option list expected")
    };
    let record_id = a.record_keys[&("main.ubi".into(), "Item".into())];
    let none = Value::Option {
        type_id: option_id,
        value: None,
    };
    let item = Value::Record {
        type_id: record_id,
        fields: [("value".into(), Value::Int(7))]
            .into_iter()
            .collect::<std::collections::BTreeMap<_, _>>()
            .into(),
    };
    let some = Value::Option {
        type_id: option_id,
        value: Some(std::rc::Rc::new(item)),
    };
    let valid = Value::List {
        type_id: list_id,
        elements: vec![none, some].into(),
    };
    assert_eq!(run(&a, "total", vec![valid]), Ok(Value::Int(7)));
    let invalid = Value::List {
        type_id: list_id,
        elements: vec![Value::Int(7)].into(),
    };
    assert_eq!(
        run(&a, "total", vec![invalid]),
        Err(InvocationError::InvalidArguments)
    );
    node(
        &a,
        r#"
const text='[{"tag":"None"},{"tag":"Some","value":{"value":7}}]';
assert.equal(m.total(text),7);
const xs=m.identity(text); assert.deepEqual(xs,[{tag:'None'},{tag:'Some',value:{value:7}}]);
assert.ok(Object.isFrozen(xs)); assert.ok(Object.isFrozen(xs[0])); assert.ok(Object.isFrozen(xs[1])); assert.ok(Object.isFrozen(xs[1].value));
assert.equal(m.total(JSON.stringify(xs)),7);
assert.deepEqual(m.nested('{"tag":"Some","value":{"tag":"None"}}'),{tag:'Some',value:{tag:'None'}});
let reads=0; const live=[{get tag(){reads++; throw Error('getter');}}];
assert.throws(()=>m.total(live),TypeError); assert.equal(reads,0);
const proxy=new Proxy([], {get(){reads++; throw Error('proxy');},ownKeys(){reads++; throw Error('proxy');}});
assert.throws(()=>m.total(proxy),TypeError); assert.equal(reads,0);
for(const bad of ['null','{}','[1]','[null]','[{"tag":"Other"}]','[{"tag":"Some"}]','[{"tag":"None","value":0}]','[{"tag":"Some","value":{"value":7},"extra":0}]','[{"tag":"Some","value":{"value":true}}]','[{"tag":"Some","value":{"value":2147483648}}]','[{"tag":"Some","value":{"value":7,"extra":0}}]','[{"tag":"Some","value":{"__proto__":7}}]','[']) assert.throws(()=>m.total(bad),TypeError);
"#,
    );
}

#[test]
fn aggregate_depth_limit_counts_lists_and_options_together() {
    // Named layers keep source parser nesting independent of aggregate depth.
    let mut source = String::from("export record R0 { value: int }\n");
    let mut body = String::from("let v0 = R0 { value: 7 }; ");
    for level in 1..=10 {
        source.push_str(&format!(
            "export record R{level} {{ child: List<Option<R{}>> }}\n",
            level - 1
        ));
        body.push_str(&format!(
            "let v{level} = R{level} {{ child: [Option.Some(v{})] }}; ",
            level - 1
        ));
    }
    source.push_str(&format!("export fn make32() -> List<R10> {{ {body} [v10] }}\nexport fn make33() -> Option<List<R10>> {{ {body} Option.Some([v10]) }}\nexport fn host32(x: List<R10>) -> int {{ 32 }}\nexport fn host33(x: Option<List<R10>>) -> int {{ 1 / 0 }}\n"));
    let a = checked(&source);
    let valid = run(&a, "make32", vec![]).unwrap();
    assert_eq!(run(&a, "host32", vec![valid.clone()]), Ok(Value::Int(32)));
    let ValueType::Option(option_id) =
        a.signatures[&("main.ubi".into(), "host33".into())].parameters[0]
    else {
        panic!("Option expected")
    };
    let invalid = Value::Option {
        type_id: option_id,
        value: Some(std::rc::Rc::new(valid)),
    };
    assert_eq!(
        run(&a, "host33", vec![invalid]),
        Err(InvocationError::InvalidArguments)
    );
    fault(&a, "make33", "UBI-R0005");
    node(
        &a,
        r#"
const made=m.make32(); assert.ok(Object.isFrozen(made)); assert.equal(m.host32(JSON.stringify(made)),32);
assert.throws(()=>m.make33(),e=>e.code==='UBI-R0005');
let x={value:7}; for(let i=0;i<10;i++) x={child:[{tag:'Some',value:x}]};
assert.equal(m.host32(JSON.stringify([x])),32); assert.throws(()=>m.host33(JSON.stringify({tag:'Some',value:[x]})),TypeError);
"#,
    );
}

#[test]
fn nested_closures_iteration_captures_and_expected_options_are_lexical() {
    let a = checked(
        r#"
fn unwrap(x: Option<int>) -> int { match (x) { Option.Some(n) => n, Option.None => 7 } }
fn emptyOption() -> Option<List<int>> { Option.Some([]) }
export fn nestedCaptures() -> int {
  let mut base = 2;
  let outer = (x: int) => (y: int) => base + x + y;
  base = 50;
  let inner = outer(3);
  inner(4)
}
export fn iterations() -> int {
  let mut saved = (n: int) => n;
  for (i in range(0, 3)) { saved = (n: int) => i + n; }
  saved(5)
}
export fn expectedTypes() -> int {
  let x: Option<List<Option<int>>> = Option.Some([Option.None]);
  let size = match (x) { Option.Some(xs) => length(xs), Option.None => 0 };
  unwrap(Option.None) + size + match (emptyOption()) { Option.Some(xs) => length(xs), Option.None => 99 }
}
export fn nestedShadow() -> int {
  let n = 1; let f = (n: int) => { let g = (x: int) => n + x; g(3) }; f(2) + n
}
"#,
    );
    for (name, expected) in [
        ("nestedCaptures", 9),
        ("iterations", 7),
        ("expectedTypes", 8),
        ("nestedShadow", 6),
    ] {
        assert_eq!(run(&a, name, vec![]), Ok(Value::Int(expected)), "{name}");
    }
    node(&a, "assert.equal(m.nestedCaptures(),9); assert.equal(m.iterations(),7); assert.equal(m.expectedTypes(),8); assert.equal(m.nestedShadow(),6);");
}

#[test]
fn aggregate_host_leaf_validation_obeys_unit_float_and_unicode_types() {
    let a = checked(
        r#"
export fn stringLength(xs: List<string>) -> int { fold(xs, 0, (n: int, s: string) => n + length(s)) }
export fn units(xs: List<Option<unit>>) -> List<Option<unit>> { xs }
export fn floating(x: Option<float>) -> float { match (x) { Option.Some(v) => v, Option.None => 0.0 } }
"#,
    );
    node(
        &a,
        r#"
assert.equal(m.stringLength('["😀","e\\u0301"]'),3);
for(const text of ['["\\ud800"]','["\\udfff"]','[0]','[true]']) assert.throws(()=>m.stringLength(text),TypeError);
const unit=m.units('[{"tag":"Some","value":null},{"tag":"None"}]');
assert.ok(Object.isFrozen(unit)); assert.ok(Object.isFrozen(unit[0]));
assert.equal(unit[0].value,undefined); assert.equal(unit[1].tag,'None');
assert.equal(JSON.stringify(unit,(_key,v)=>v===undefined?null:v),'[{"tag":"Some","value":null},{"tag":"None"}]');
for(const text of ['[{"tag":"Some","value":0}]','[{"tag":"Some","value":"null"}]']) assert.throws(()=>m.units(text),TypeError);
for(const [tag,value] of [['NaN',NaN],['+Infinity',Infinity],['-Infinity',-Infinity],['+0',0],['-0',-0]]) assert.ok(Object.is(m.floating(JSON.stringify({tag:'Some',value:tag})),value));
for(const value of ['Infinity','1.0',true,{},null]) assert.throws(()=>m.floating(JSON.stringify({tag:'Some',value})),TypeError);
"#,
    );
}
