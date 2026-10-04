//! Independent fixed expectations from SPEC sections 3, 6, 14, 15 and 16.
use crate::analyzer::{analyze, Analysis, ValueType};
use crate::interpreter::{invoke, InvocationError, Value};
use crate::source::{SourceFile, SourceSet};
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
    let a = analysis(items);
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

fn rejected(items: &[(&str, &str)], code: &str) {
    let a = analysis(items);
    assert!(
        a.diagnostics.iter().any(|d| d.code == code),
        "{items:?}: {:#?}",
        a.diagnostics
    );
    assert!(crate::ir::lower(&a).is_err(), "invalid program lowered");
}

fn node(a: &Analysis, assertions: &str) {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "ubi-enums-{}-{}",
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
        .expect("Node required for enum conformance");
    fs::remove_dir_all(&path).unwrap();
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

fn enum_argument(a: &Analysis, function: &str, tag: &str, values: Vec<Value>) -> Value {
    let ValueType::Enum(type_id) =
        a.signatures[&("main.ubi".into(), function.into())].parameters[0]
    else {
        panic!("enum argument expected")
    };
    Value::Enum {
        type_id,
        tag: tag.to_owned().into(),
        values: values.into(),
    }
}

#[test]
fn enums_preserve_aliases_structural_equality_and_all_payloads() {
    let a = checked(&[(
        "main.ubi",
        r#"
enum Shape { Point, Rectangle(int, int), Label(string), }
record Holder { shape: Shape, extra: Option<List<Shape>> }
enum Real { Value(float) }
fn area(shape: Shape) -> int {
  match (shape) { Shape.Point => 0, Shape.Rectangle(w, h) => w * h, Shape.Label(s) => length(s) }
}
export fn sample() -> int { area(Shape.Rectangle(3, 4)) + area(Shape.Label("a😀")) + area(Shape.Point) }
export fn aliases() -> bool {
  let mut current = Shape.Rectangle(2, 3); let alias = current;
  let snapshot = (n: int) => area(current) + n;
  current = Shape.Point;
  alias == Shape.Rectangle(2, 3) && current == Shape.Point && snapshot(1) == 7
}
export fn equality() -> bool {
  let a = Holder { shape: Shape.Point, extra: Option.Some([Shape.Rectangle(2, 3), Shape.Label("😀")]) };
  let b = Holder { extra: Option.Some([Shape.Rectangle(2, 3), Shape.Label("😀")]), shape: Shape.Point };
  a == b && Shape.Rectangle(2, 3) != Shape.Rectangle(2, 4)
    && Shape.Rectangle(2, 3) != Shape.Rectangle(3, 2) && Shape.Point != Shape.Label("")
    && Real.Value(0.0) == Real.Value(-0.0)
}
export fn nan() -> bool { let x = Real.Value(0.0 / 0.0); let alias = x; x == alias }
export fn mapped() -> int { fold(map([Shape.Point, Shape.Rectangle(2, 3)], area), 0, (s: int, x: int) => s + x) }
"#,
    )]);
    for (name, value) in [
        ("sample", Value::Int(14)),
        ("aliases", Value::Bool(true)),
        ("equality", Value::Bool(true)),
        ("nan", Value::Bool(false)),
        ("mapped", Value::Int(6)),
    ] {
        assert_eq!(run(&a, name, vec![]), Ok(value), "{name}");
    }
    node(&a, "assert.equal(m.sample(),14); assert.equal(m.aliases(),true); assert.equal(m.equality(),true); assert.equal(m.nan(),false); assert.equal(m.mapped(),6);");
}

#[test]
fn nested_payload_matrix_covers_combinations_and_first_matching_arm() {
    let a = checked(&[(
        "main.ubi",
        r#"
enum Bit { Off, On }
enum Pair { Empty, Bits(Bit, Option<bool>) }
fn score(p: Pair) -> int {
  match (p) {
    Pair.Empty => 0,
    Pair.Bits(Bit.Off, Option.None) => 1,
    Pair.Bits(Bit.On, Option.None) => 2,
    Pair.Bits(Bit.Off, Option.Some(true)) => 3,
    Pair.Bits(Bit.Off, Option.Some(false)) => 4,
    Pair.Bits(Bit.On, Option.Some(true)) => 5,
    Pair.Bits(Bit.On, Option.Some(false)) => 6,
  }
}
export fn matrix() -> int {
  score(Pair.Empty) + score(Pair.Bits(Bit.Off, Option.None))
    + score(Pair.Bits(Bit.On, Option.None)) * 10
    + score(Pair.Bits(Bit.Off, Option.Some(true))) * 100
    + score(Pair.Bits(Bit.Off, Option.Some(false))) * 1000
    + score(Pair.Bits(Bit.On, Option.Some(true))) * 10000
    + score(Pair.Bits(Bit.On, Option.Some(false))) * 100000
}
export fn precedence() -> int {
  match (Pair.Bits(Bit.On, Option.Some(true))) {
    Pair.Bits(_, Option.Some(true)) => 7,
    Pair.Bits(Bit.On, _) => 1 / 0,
    _ => 1 / 0,
  }
}
export fn shadow() -> int {
  let Pair = 11; let x = 8;
  match (make()) { Pair.Empty => 0, Pair.Bits(_, Option.Some(x)) => if (x) { Pair } else { 0 }, Pair.Bits(_, Option.None) => x }
}
fn make() -> Pair { Pair.Bits(Bit.On, Option.Some(true)) }
"#,
    )]);
    assert_eq!(run(&a, "matrix", vec![]), Ok(Value::Int(654321)));
    assert_eq!(run(&a, "precedence", vec![]), Ok(Value::Int(7)));
    assert_eq!(run(&a, "shadow", vec![]), Ok(Value::Int(11)));
    node(&a, "assert.equal(m.matrix(),654321); assert.equal(m.precedence(),7); assert.equal(m.shadow(),11);");
    for source in [
        "enum E { Pair(bool, bool) } fn f(e: E) -> int { match (e) { E.Pair(true, true) => 1, E.Pair(false, false) => 2 } }",
        "enum E { Pair(bool, bool) } fn f(e: E) -> int { match (e) { E.Pair(true, _) => 1, E.Pair(_, true) => 2 } }",
        "enum E { Pair(bool, bool) } fn f(e: E) -> int { match (e) { E.Pair(true, _) => 1, E.Pair(false, _) if (true) => 2 } }",
        "enum Bit { Off, On } enum Pair { Both(Bit, Option<bool>) } fn f(e: Pair) -> int { match (e) { Pair.Both(Bit.Off, _) => 1, Pair.Both(_, Option.None) => 2, Pair.Both(Bit.On, Option.Some(true)) => 3 } }",
    ] {
        rejected(&[("main.ubi", source)], "UBI0020");
    }
}

#[test]
fn constructors_and_guards_evaluate_once_left_to_right_and_stop_at_faults() {
    let a = checked(&[(
        "main.ubi",
        r#"
export enum Pair { Values(int, int), Empty }
export fn order() -> int {
  let mut n = 0;
  let pair = Pair.Values({ n = n + 1; n }, { n = n + 1; n });
  let result = match ({ n = n + 1; pair }) {
    Pair.Values(a, b) if ({ n = n + 1; false }) => 1 / 0,
    Pair.Values(a, b) if ({ n = n + 1; true }) => a * 10 + b,
    Pair.Values(_, _) => 1 / 0, Pair.Empty => 1 / 0,
  };
  n * 100 + result
}
export fn firstFault() -> Pair { Pair.Values(1 / 0, 2147483647 + 1) }
export fn secondFault() -> Pair { Pair.Values(2147483647 + 1, 1 / 0) }
export fn earlyReturn() -> int { let p = Pair.Values({ return 17; 0 }, 1 / 0); 0 }
export fn guardReturn() -> int { match (Pair.Empty) { Pair.Empty if ({ return 19; false }) => 1 / 0, _ => 1 / 0 } }
"#,
    )]);
    assert_eq!(run(&a, "order", vec![]), Ok(Value::Int(512)));
    assert_eq!(run(&a, "earlyReturn", vec![]), Ok(Value::Int(17)));
    assert_eq!(run(&a, "guardReturn", vec![]), Ok(Value::Int(19)));
    fault(&a, "firstFault", "UBI-R0002");
    fault(&a, "secondFault", "UBI-R0001");
    node(&a, "assert.equal(m.order(),512); assert.equal(m.earlyReturn(),17); assert.equal(m.guardReturn(),19); assert.throws(()=>m.firstFault(),e=>e.code==='UBI-R0002'); assert.throws(()=>m.secondFault(),e=>e.code==='UBI-R0001');");
}

#[test]
fn invalid_enum_constructors_patterns_and_bindings_reject_before_lowering() {
    for source in [
        "enum E { A } fn f() { E.Unknown; }",
        "enum E { A(int) } fn f() { E.A; }",
        "enum E { A } fn f() { E.A(); }",
        "enum E { A(int) } fn f() { E.A(true); }",
        "enum E { A } enum F { A } fn f() { E.A == F.A; }",
        "enum E { A } enum F { A } fn f(e: E) { match (e) { F.A => 0 }; }",
        "enum E { A } fn f(e: E) { match (e) { E.Unknown => 0, _ => 1 }; }",
        "enum E { A(int, int) } fn f(e: E) { match (e) { E.A(x) => x }; }",
        "enum E { A(int) } fn f(e: E) { match (e) { E.A(true) => 1, _ => 0 }; }",
        "enum E { A(int, int) } fn f(e: E) { match (e) { E.A(x, x) => x }; }",
        "enum E { A } fn f(e: E) { match (e) { _ => 1, E.A => false }; }",
        "enum E { A } fn f(e: E) { match (e) { _ => 1, E.Unknown => 1 }; }",
        "enum E { A } fn f() { let e: E<int> = E.A; }",
    ] {
        rejected(&[("main.ubi", source)], "UBI0020");
    }
    // Shadowing makes E.A ordinary member access, governed by the record-field diagnostic.
    rejected(
        &[("main.ubi", "enum E { A } fn f() { let E = 1; E.A; }")],
        "UBI0030",
    );
    for source in [
        "enum E { A(int) } fn f() { E.A(); }",
        "enum E { A(int) } fn f() { E.A(1, 2); }",
    ] {
        rejected(&[("main.ubi", source)], "UBI0023");
    }
    for source in [
        "enum E { A(int) } fn f(e: E) { match (e) { E.A(x) => { x = 2; x } }; }",
        "enum E { A } fn f() { let e = E.A; e = E.A; }",
    ] {
        rejected(&[("main.ubi", source)], "UBI0022");
    }
    rejected(
        &[(
            "main.ubi",
            "enum E { A(int) } fn f(e: E) { match (e) { E.A(x) => x }; x; }",
        )],
        "UBI0010",
    );
    for source in [
        "enum E { A, A }",
        "enum E { A } record E {}",
        "enum E { A } fn E() {}",
        "enum Option { A }",
    ] {
        rejected(&[("main.ubi", source)], "UBI0011");
    }
    for source in ["enum E {}", "enum E { A() }", "enum E { A(int,,bool) }"] {
        rejected(&[("main.ubi", source)], "UBI0002");
    }
    rejected(&[("main.ubi", "enum E<T> { A(T) }")], "UBI0003");
}

#[test]
fn imports_preserve_nominality_and_exported_types_cannot_leak_private_types() {
    let a = checked(&[
        ("shapes.ubi", "export enum Shape { Point, Rectangle(int, int) } export fn make() -> Shape { Shape.Rectangle(3, 4) } export fn area(s: Shape) -> int { match (s) { Shape.Point => 0, Shape.Rectangle(w, h) => w * h } }"),
        ("main.ubi", "import { Shape, make, area } from \"./shapes.ubi\"; export fn run() -> int { area(make()) + area(Shape.Rectangle(2, 5)) } export fn same() -> bool { make() == Shape.Rectangle(3, 4) }"),
    ]);
    assert_eq!(run(&a, "run", vec![]), Ok(Value::Int(22)));
    assert_eq!(run(&a, "same", vec![]), Ok(Value::Bool(true)));
    node(&a, "assert.equal(m.run(),22); assert.equal(m.same(),true);");
    rejected(&[
        ("a.ubi", "export enum E { A } export fn make() -> E { E.A }"),
        ("b.ubi", "export enum E { A } export fn take(e: E) -> int { 1 }"),
        ("main.ubi", "import { make } from \"./a.ubi\"; import { take } from \"./b.ubi\"; export fn run() -> int { take(make()) }"),
    ], "UBI0020");
    rejected(
        &[
            ("a.ubi", "enum E { A }"),
            ("main.ubi", "import { E } from \"./a.ubi\";"),
        ],
        "UBI0012",
    );
    for source in [
        "enum Hidden { A } export fn f() -> Hidden { Hidden.A }",
        "enum Hidden { A } export fn f(x: List<Option<Hidden>>) -> int { 0 }",
        "enum Hidden { A } export record Public { x: Hidden }",
        "record Hidden { x: int } export enum Public { A(List<Option<Hidden>>) }",
        "enum Hidden { A } export enum Public { A(Hidden) }",
    ] {
        rejected(&[("main.ubi", source)], "UBI0013");
    }
}

#[test]
fn host_enum_arguments_validate_exact_tags_payloads_and_freeze_nested_values() {
    let a = checked(&[(
        "main.ubi",
        r#"
export record Item { value: int }
export enum Input { Empty, Data(int, string, float, unit, List<Option<Item>>) }
export enum Other { Empty, Data(int, string, float, unit, List<Option<Item>>) }
export fn accept(x: Input) -> int { match (x) { Input.Empty => 0, Input.Data(n, _, _, _, _) => n } }
export fn echo(x: Input) -> Input { x }
export fn real(x: Input) -> float { match (x) { Input.Empty => 0.0, Input.Data(_, _, n, _, _) => n } }
export fn other(x: Other) -> int { 0 }
export fn constructed() -> Input { Input.Data(7, "😀", -0.0, (), [Option.Some(Item { value: 4 })]) }
"#,
    )]);
    let empty = enum_argument(&a, "accept", "Empty", vec![]);
    assert_eq!(run(&a, "accept", vec![empty.clone()]), Ok(Value::Int(0)));
    assert_eq!(run(&a, "echo", vec![empty.clone()]), Ok(empty));
    let data = run(&a, "constructed", vec![]).unwrap();
    assert_eq!(run(&a, "accept", vec![data]), Ok(Value::Int(7)));
    for invalid in [
        enum_argument(&a, "accept", "Missing", vec![]),
        enum_argument(&a, "accept", "Empty", vec![Value::Int(0)]),
        enum_argument(&a, "accept", "Data", vec![]),
        enum_argument(&a, "other", "Empty", vec![]),
        Value::Int(0),
    ] {
        assert_eq!(
            run(&a, "accept", vec![invalid]),
            Err(InvocationError::InvalidArguments)
        );
    }
    node(
        &a,
        r#"
const text='{"tag":"Data","values":[7,"😀","-0",null,[{"tag":"Some","value":{"value":4}}]]}';
assert.equal(m.accept(text),7); assert.equal(m.accept('{"tag":"Empty","values":[]}'),0);
const output=m.echo(text); assert.deepEqual(output,{tag:'Data',values:[7,'😀',-0,undefined,[{tag:'Some',value:{value:4}}]]});
for (const value of [output, output.values, output.values[4], output.values[4][0], output.values[4][0].value]) assert.ok(Object.isFrozen(value));
const serialized=JSON.stringify(output,(_key,value)=>value===undefined?null:Object.is(value,-0)?'-0':value);
assert.equal(m.accept(serialized),7); assert.ok(Object.is(m.real(serialized),-0));
assert.ok(Object.isFrozen(m.constructed().values));
for (const [tag,expected] of [['NaN',NaN],['+Infinity',Infinity],['-Infinity',-Infinity],['+0',0],['-0',-0]]) {
  const x=JSON.parse(text); x.values[2]=tag; assert.ok(Object.is(m.real(JSON.stringify(x)),expected));
  x.values[1]=tag; assert.equal(m.echo(JSON.stringify(x)).values[1],tag);
}
let reads=0; const live={get tag(){reads++; throw Error('getter');}};
const proxy=new Proxy({}, {get(){reads++; throw Error('proxy');},ownKeys(){reads++; throw Error('proxy');},getOwnPropertyDescriptor(){reads++; throw Error('proxy');}});
assert.throws(()=>m.accept(live),TypeError); assert.throws(()=>m.accept(proxy),TypeError); assert.equal(reads,0);
for(const bad of ['null','[]','{}','{"tag":"Empty"}','{"values":[]}','{"tag":"Missing","values":[]}','{"tag":"Empty","values":[0]}','{"tag":"Empty","values":[],"extra":0}','{"tag":"Data","values":[]}','{"tag":false,"values":[]}','{"tag":"Empty","values":{}}','{"tag":"Empty","values":[],"__proto__":0}','{']) assert.throws(()=>m.accept(bad),TypeError);
for(const [index,bad] of [[0,true],[0,2147483648],[1,1],[1,'\ud800'],[2,true],[2,'oops'],[3,'null'],[4,[{tag:'Some',value:{value:true}}]],[4,[{tag:'None',value:0}]]]) {
  const x=JSON.parse(text); x.values[index]=bad; assert.throws(()=>m.accept(JSON.stringify(x)),TypeError);
}
"#,
    );
}

#[test]
fn prototype_named_variants_are_ordinary_declared_tags() {
    let a = checked(&[(
        "main.ubi",
        r#"
export enum Special { __proto__, constructor(int), toString }
export fn take(x: Special) -> int {
  match (x) { Special.__proto__ => 1, Special.constructor(n) => n, Special.toString => 3 }
}
export fn make() -> Special { Special.__proto__ }
"#,
    )]);
    for (tag, values, expected) in [
        ("__proto__", vec![], 1),
        ("constructor", vec![Value::Int(9)], 9),
        ("toString", vec![], 3),
    ] {
        assert_eq!(
            run(&a, "take", vec![enum_argument(&a, "take", tag, values)]),
            Ok(Value::Int(expected))
        );
    }
    node(
        &a,
        r#"
assert.deepEqual(m.make(),{tag:'__proto__',values:[]});
assert.equal(m.take('{"tag":"__proto__","values":[]}'),1);
assert.equal(m.take('{"tag":"constructor","values":[9]}'),9);
assert.equal(m.take('{"tag":"toString","values":[]}'),3);
for (const tag of ['hasOwnProperty','valueOf','prototype']) assert.throws(()=>m.take(JSON.stringify({tag,values:[]})),TypeError);
"#,
    );
}

#[test]
fn recursive_enums_and_mutual_record_references_keep_nominal_values() {
    let a = checked(&[(
        "main.ubi",
        r#"
enum Tree { Leaf(int), Branch(Tree, Tree) }
record Link { next: Chain }
enum Chain { End, More(int, Link) }
fn total(t: Tree) -> int { match (t) { Tree.Leaf(n) => n, Tree.Branch(a, b) => total(a) + total(b) } }
fn sum(c: Chain) -> int { match (c) { Chain.End => 0, Chain.More(n, link) => n + sum(link.next) } }
export fn trees() -> int { total(Tree.Branch(Tree.Leaf(2), Tree.Branch(Tree.Leaf(3), Tree.Leaf(4)))) }
export fn mutual() -> int { sum(Chain.More(5, Link { next: Chain.More(7, Link { next: Chain.End }) })) }
export fn same() -> bool {
  let x = Tree.Branch(Tree.Leaf(1), Tree.Leaf(2));
  x == Tree.Branch(Tree.Leaf(1), Tree.Leaf(2)) && x != Tree.Branch(Tree.Leaf(2), Tree.Leaf(1))
}
"#,
    )]);
    assert_eq!(run(&a, "trees", vec![]), Ok(Value::Int(9)));
    assert_eq!(run(&a, "mutual", vec![]), Ok(Value::Int(12)));
    assert_eq!(run(&a, "same", vec![]), Ok(Value::Bool(true)));
    node(
        &a,
        "assert.equal(m.trees(),9); assert.equal(m.mutual(),12); assert.equal(m.same(),true);",
    );
}

#[test]
fn enum_depth_counts_payload_values_but_not_host_envelope_arrays() {
    let a = checked(&[(
        "main.ubi",
        r#"
export enum Chain { End, Next(Chain) }
export fn make32() -> Chain { let mut c = Chain.End; for (n in range(0, 31)) { c = Chain.Next(c); } c }
export fn make33() -> Chain { let mut c = Chain.End; for (n in range(0, 32)) { c = Chain.Next(c); } c }
export fn host(x: Chain) -> int { 32 }
"#,
    )]);
    let valid = run(&a, "make32", vec![]).unwrap();
    assert_eq!(run(&a, "host", vec![valid.clone()]), Ok(Value::Int(32)));
    let invalid = enum_argument(&a, "host", "Next", vec![valid]);
    assert_eq!(
        run(&a, "host", vec![invalid]),
        Err(InvocationError::InvalidArguments)
    );
    fault(&a, "make33", "UBI-R0005");
    node(
        &a,
        r#"
const value=m.make32(); assert.ok(Object.isFrozen(value)); assert.ok(Object.isFrozen(value.values));
assert.equal(m.host(JSON.stringify(value)),32); assert.throws(()=>m.make33(),e=>e.code==='UBI-R0005');
let x={tag:'End',values:[]}; for(let i=0;i<31;i++) x={tag:'Next',values:[x]};
assert.equal(m.host(JSON.stringify(x)),32); x={tag:'Next',values:[x]}; assert.throws(()=>m.host(JSON.stringify(x)),TypeError);
"#,
    );
}

#[test]
fn mixed_enum_record_list_option_depth_is_shared() {
    let a = checked(&[(
        "main.ubi",
        r#"
export record Link { children: List<Option<Chain>> }
export enum Chain { End, Next(Link) }
export fn make32() -> Option<Option<List<Chain>>> {
  let mut c = Chain.End;
  for (n in range(0, 7)) { c = Chain.Next(Link { children: [Option.Some(c)] }); }
  Option.Some(Option.Some([Chain.Next(Link { children: [] }), c, Chain.End]))
}
export fn host32(x: Option<Option<List<Chain>>>) -> int { 32 }
export fn make33() -> Chain {
  let mut c = Chain.End;
  for (n in range(0, 8)) { c = Chain.Next(Link { children: [Option.Some(c)] }); }
  c
}
"#,
    )]);
    // End is one level; seven wrappers add four each; list and two Options add three = 32.
    let valid = run(&a, "make32", vec![]).unwrap();
    assert_eq!(run(&a, "host32", vec![valid]), Ok(Value::Int(32)));
    fault(&a, "make33", "UBI-R0005");
    node(&a, "assert.equal(m.host32(JSON.stringify(m.make32())),32); assert.throws(()=>m.make33(),e=>e.code==='UBI-R0005');");
}

#[test]
fn exhaustive_pattern_matrix_work_is_bounded_with_a_valid_small_control() {
    fn source(width: usize) -> String {
        let types = vec!["bool"; width].join(", ");
        let mut arms = Vec::new();
        for position in 0..width {
            let mut fields = vec!["_"; width];
            fields[position] = "true";
            arms.push(format!(
                "Bits.Row({}) => {}",
                fields.join(", "),
                position + 1
            ));
        }
        arms.push(format!(
            "Bits.Row({}) => 0",
            vec!["false"; width].join(", ")
        ));
        let mut value = vec!["false"; width];
        value[1] = "true";
        format!(
            "enum Bits {{ Row({types}) }} fn score(x: Bits) -> int {{ match (x) {{ {} }} }} export fn run() -> int {{ score(Bits.Row({})) }}",
            arms.join(", "), value.join(", ")
        )
    }
    // Every assignment has a first true bit or is all false; coverage is exhaustive.
    // Large independent boolean columns require more than the permitted matrix work.
    let small = source(4);
    let a = checked(&[("main.ubi", &small)]);
    assert_eq!(run(&a, "run", vec![]), Ok(Value::Int(2)));
    node(&a, "assert.equal(m.run(),2);");
    let large = source(128);
    rejected(&[("main.ubi", &large)], "UBI0090");
    let a = analysis(&[("main.ubi", &large)]);
    assert!(a
        .diagnostics
        .iter()
        .any(|d| d.code == "UBI0090" && d.primary.start == large.find("match").unwrap()));
}
