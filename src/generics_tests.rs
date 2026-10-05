//! Independently authored conformance from SPEC section 17.
use crate::analyzer::{analyze, Analysis};
use crate::interpreter::{invoke, InvocationError, Value};
use crate::source::{SourceFile, SourceSet};
use std::fs;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

pub(super) fn analysis(items: &[(&str, &str)]) -> Analysis {
    let mut sources = SourceSet::default();
    for (id, source) in items {
        sources
            .insert(SourceFile::new(id, source.as_bytes().to_vec()).unwrap())
            .unwrap();
    }
    analyze(&sources)
}

pub(super) fn checked(items: &[(&str, &str)]) -> Analysis {
    let a = analysis(items);
    assert!(a.diagnostics.is_empty(), "{:#?}", a.diagnostics);
    a
}

pub(super) fn run(a: &Analysis, name: &str, args: Vec<Value>) -> Result<Value, InvocationError> {
    invoke(a, &("main.ubi".into(), name.into()), args)
}

pub(super) fn rejected(items: &[(&str, &str)], code: &str) {
    let a = analysis(items);
    assert!(
        a.diagnostics.iter().any(|d| d.code == code),
        "{items:?}: {:#?}",
        a.diagnostics
    );
    assert!(crate::ir::lower(&a).is_err(), "invalid program lowered");
}

pub(super) fn node(a: &Analysis, assertions: &str) {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "ubi-next-conformance-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&path).unwrap();
    for (id, source) in crate::js_backend::emit(&crate::ir::lower(a).unwrap()).unwrap() {
        let file = path.join(id);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, source).unwrap();
    }
    fs::write(
        path.join("test.mjs"),
        format!(
        "import assert from 'node:assert/strict';\nimport * as m from './main.mjs';\n{assertions}"
    ),
    )
    .unwrap();
    let result = Command::new("node")
        .arg(path.join("test.mjs"))
        .output()
        .expect("Node required for independent conformance");
    fs::remove_dir_all(&path).unwrap();
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn generic_functions_records_enums_and_callbacks_have_fixed_values() {
    let a = checked(&[(
        "main.ubi",
        r#"
record Box<T> { value: T }
enum Choice<T> { Empty, Present(T) }
fn identity<T>(x: T) -> T { x }
fn box<T>(x: T) -> Box<T> { Box { value: x } }
fn unwrap<T>(x: Choice<T>, fallback: T) -> T {
  match (x) { Choice.Empty => fallback, Choice.Present(value) => value }
}
fn count<T>(xs: List<T>) -> int { length(xs) }
export fn values() -> int {
  let a = box(7); let b = box("😀");
  a.value + length(b.value) + unwrap(Choice.Present(3), 0)
    + count(map([2, 4], identity))
}
export fn concreteEquality() -> bool {
  Box { value: 4 } == Box { value: 4 }
    && Choice.Present("a") == Choice.Present("a")
}
export fn captures() -> int {
  snapshot(7, 20) + snapshot(3, 99)
}
export fn functionValue() -> int {
  let callback = identity((x: int) => x + 1); callback(8)
}
fn snapshot<T>(before: T, after: T) -> T {
  let mut x = before; let take = () => x; x = after; take()
}
"#,
    )]);
    assert_eq!(run(&a, "values", vec![]), Ok(Value::Int(13)));
    assert_eq!(run(&a, "concreteEquality", vec![]), Ok(Value::Bool(true)));
    assert_eq!(run(&a, "captures", vec![]), Ok(Value::Int(10)));
    assert_eq!(run(&a, "functionValue", vec![]), Ok(Value::Int(9)));
    node(&a, "assert.equal(m.values(),13); assert.equal(m.concreteEquality(),true); assert.equal(m.captures(),10); assert.equal(m.functionValue(),9);");
}

#[test]
fn expected_types_resolve_empty_lists_none_and_payload_free_generics() {
    let a = checked(&[(
        "main.ubi",
        r#"
record Box<T> { value: T }
enum Choice<T> { Empty, Present(T) }
fn identity<T>(x: T) -> T { x }
fn empty<T>() -> List<T> { [] }
fn none<T>() -> Option<T> { Option.None }
fn absent<T>() -> Choice<T> { Choice.Empty }
fn accept(x: Box<List<int>>) -> int { length(x.value) }
fn tail() -> List<int> { identity([]) }
fn early() -> Option<int> { return none(); }
fn branch(b: bool) -> Choice<List<int>> {
  if (b) { Choice.Present([]) } else { absent() }
}
export fn contexts() -> bool {
  let xs: List<int> = empty(); let no: Option<int> = identity(Option.None);
  let chosen: Choice<int> = identity(Choice.Empty);
  let boxed: Box<Option<List<int>>> = Box { value: Option.Some([]) };
  accept(Box { value: [] }) == 0 && xs == [] && no == Option.None
    && chosen == Choice.Empty && boxed.value == Option.Some([])
    && tail() == [] && early() == Option.None
    && branch(true) == Choice.Present([]) && branch(false) == Choice.Empty
}
"#,
    )]);
    assert_eq!(run(&a, "contexts", vec![]), Ok(Value::Bool(true)));
    node(&a, "assert.equal(m.contexts(),true);");
}

#[test]
fn generic_bodies_are_universal_and_inference_is_invariant() {
    for source in [
        "fn bad<T>(x: T) -> T { x + x } export fn run() -> int { bad(1) }",
        "fn bad<T>(x: T) -> bool { x == x } export fn run() -> bool { bad(1) }",
        "fn bad<T>(x: T) -> bool { x < x } export fn run() -> bool { bad(1) }",
        "fn bad<T>(x: T) -> int { length(x) } export fn run() -> int { bad(\"x\") }",
        "fn same<T>(a: T, b: T) -> T { a } fn run() -> int { same(1, true) }",
        "fn same<T>(a: List<T>, b: T) -> T { b } fn run() -> int { same([1], true) }",
        "fn id<T>(x: T) -> T { x } fn run() -> string { id(1) }",
        "record Box<T> { value: T } fn run() -> Box<string> { Box { value: 1 } }",
        "enum E<T> { A(T), Empty } fn run() -> E<string> { E.A(1) }",
        "record Box<T> { value: T } fn run(a: Box<int>, b: Box<string>) -> bool { a == b }",
        "record Box<T> { value: T } fn run(a: Box<int>) -> Box<string> { Box { ...a, value: \"x\" } }",
        "fn id<T>(x: T) -> T { x } fn run() { let f = id; }",
        "fn empty<T>() -> List<T> { [] } fn run() { empty(); }",
        "record Box<T> { value: T } fn run() { Box { value: [] }; }",
        "enum E<T> { Empty } fn run() { E.Empty; }",
        "record Box<T> { value: T } fn run(x: Box) {}",
        "record Box<T> { value: T } fn run(x: Box<int, int>) {}",
        "fn bad<Option>(x: Option) -> Option { x }",
        "fn wrap<T>(x: T) -> List<T> { [x] } fn run() { wrap((n: int) => n); }",
        "fn wrap<T>(x: T) -> Option<T> { Option.Some(x) } fn run() { wrap((n: int) => n); }",
        "record Box<T> { value: T } fn wrap<T>(x: T) -> Box<T> { Box { value: x } } fn run() { wrap((n: int) => n); }",
        "enum E<T> { A(T) } fn wrap<T>(x: T) -> E<T> { E.A(x) } fn run() { wrap((n: int) => n); }",
        "fn wrap<T>(x: T) -> List<T> { [x] } fn run() { wrap(range(0,1)); }",
    ] {
        rejected(&[("main.ubi", source)], "UBI0020");
    }
    rejected(&[("main.ubi", "fn id<T>(x: T) { x }")], "UBI0021");
    for source in [
        "fn f<T,T>(x: T) -> T { x }",
        "record R<T,T> { x: T }",
        "enum E<T,T> { A(T) }",
    ] {
        rejected(&[("main.ubi", source)], "UBI0011");
    }
}

#[test]
fn generic_exports_imports_preserve_identity_and_private_type_rules() {
    let a = checked(&[
        ("box.ubi", "export record Box<T> { value: T } export enum E<T> { Empty, A(T) } export fn id<T>(x: T) -> T { x } export fn make<T>(x: T) -> Box<T> { Box { value: x } }"),
        ("main.ubi", "import { Box, E, id, make } from \"./box.ubi\"; export fn imported() -> int { let a: Box<int> = make(id(8)); let e: E<int> = E.A(3); a.value + match (e) { E.A(n) => n, E.Empty => 0 } }"),
    ]);
    assert_eq!(run(&a, "imported", vec![]), Ok(Value::Int(11)));
    node(&a, "assert.equal(m.imported(),11); const generic=await import('./box.mjs'); assert.throws(()=>generic.id(1),TypeError); assert.throws(()=>generic.make(1),TypeError);");
    assert!(invoke(&a, &("box.ubi".into(), "id".into()), vec![Value::Int(1)]).is_err());
    rejected(&[
        ("a.ubi", "export record Box<T> { value: T } export fn make() -> Box<int> { Box { value: 1 } }"),
        ("b.ubi", "export record Box<T> { value: T } export fn take(x: Box<int>) -> int { x.value }"),
        ("main.ubi", "import { make } from \"./a.ubi\"; import { take } from \"./b.ubi\"; export fn f() -> int { take(make()) }"),
    ], "UBI0020");
    for source in [
        "record Hidden {} export record Box<T> { value: Option<Hidden> }",
        "record Hidden {} export enum E<T> { A(List<Hidden>, T) }",
        "record Hidden {} record Box<T> { value: T } export fn f<T>(x: Box<Hidden>, y: T) -> T { y }",
        "record Hidden {} export record Box<T> { value: T } export fn f<T>(x: Box<List<Hidden>>, y: T) -> T { y }",
    ] { rejected(&[("main.ubi", source)], "UBI0013"); }
}

#[test]
fn recursive_generic_types_terminate_and_growing_instantiation_has_a_budget() {
    let a = checked(&[(
        "main.ubi",
        r#"
enum Tree<T> { Leaf(T), Branch(Tree<T>, Tree<T>) }
record Link<T> { next: Chain<T> }
enum Chain<T> { End, More(T, Link<T>) }
fn total(t: Tree<int>) -> int { match (t) { Tree.Leaf(n) => n, Tree.Branch(a,b) => total(a) + total(b) } }
fn sum(c: Chain<int>) -> int { match (c) { Chain.End => 0, Chain.More(n,link) => n + sum(link.next) } }
export fn recursive() -> int {
  total(Tree.Branch(Tree.Leaf(2), Tree.Leaf(3)))
    + sum(Chain.More(7, Link { next: Chain.More(11, Link { next: Chain.End }) }))
}
"#,
    )]);
    assert_eq!(run(&a, "recursive", vec![]), Ok(Value::Int(23)));
    node(&a, "assert.equal(m.recursive(),23);");
    rejected(
        &[(
            "main.ubi",
            "record Grow<T> { next: Option<Grow<List<T>>> } export fn f(x: Grow<int>) -> int { 0 }",
        )],
        "UBI0090",
    );
}

#[test]
fn concrete_generic_host_wrappers_validate_and_recursively_freeze() {
    let a = checked(&[(
        "main.ubi",
        r#"
export record Box<T> { value: T }
export enum E<T> { Empty, A(T) }
export fn id<T>(x: T) -> T { x }
export fn genericFault<T>(x: T) -> int { 1 / 0 }
export fn echo(x: Box<List<E<Option<int>>>>) -> Box<List<E<Option<int>>>> { id(x) }
export fn ints(x: Box<int>) -> int { x.value }
export fn text(x: Box<string>) -> string { x.value }
export fn make() -> Box<List<E<Option<int>>>> { Box { value: [E.A(Option.Some(7)), E.Empty] } }
"#,
    )]);
    let input = run(&a, "make", vec![]).unwrap();
    assert_eq!(run(&a, "echo", vec![input.clone()]), Ok(input));
    assert!(run(&a, "id", vec![Value::Int(1)]).is_err());
    let rejected = run(&a, "genericFault", vec![Value::Int(1)]);
    assert!(rejected.is_err() && !matches!(rejected, Err(InvocationError::Runtime(_))));
    assert_eq!(
        run(&a, "echo", vec![Value::Int(1)]),
        Err(InvocationError::InvalidArguments)
    );
    node(
        &a,
        r#"
assert.throws(()=>m.id(1),TypeError);
assert.throws(()=>m.genericFault(1),TypeError);
assert.throws(()=>m.ints('{"value":7}',{steps:1000000,depth:0}),TypeError);
assert.throws(()=>m.make({steps:1000000,depth:0}),TypeError);
assert.equal(m.ints('{"value":7}'),7); assert.equal(m.text('{"value":"😀"}'),'😀');
assert.throws(()=>m.text('{"value":7}'),TypeError); assert.throws(()=>m.ints('{"value":"7"}'),TypeError);
const good='{"value":[{"tag":"A","values":[{"tag":"Some","value":7}]},{"tag":"Empty","values":[]}]}';
const out=m.echo(good); assert.deepEqual(out,m.make());
for(const x of [out,out.value,out.value[0],out.value[0].values,out.value[0].values[0]]) assert.ok(Object.isFrozen(x));
for(const bad of [out,'{}','{"value":[],"extra":0}','{"value":[{"tag":"A","values":[{"tag":"Some","value":true}]}]}','{"value":[{"tag":"Empty","values":[0]}]}']) assert.throws(()=>m.echo(bad),TypeError);
"#,
    );
}
