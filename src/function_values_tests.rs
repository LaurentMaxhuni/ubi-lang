//! Independent conformance for SPEC section 23.
use crate::analyzer::ValueType;
use crate::generics_tests::{analysis, checked, node, rejected, run};
use crate::interpreter::{InvocationError, Value};
use std::rc::Rc;

#[test]
fn typed_callback_parameter_has_fixed_result() {
    let a = checked(&[("main.ubi", "fn apply(f: fn(int) -> int, x: int) -> int { f(x) } export fn main() -> int { apply((x: int) => x + 1, 41) }")]);
    assert_eq!(run(&a, "main", vec![]), Ok(Value::Int(42)));
    node(&a, "assert.equal(m.main(),42);");
}

#[test]
fn returned_nested_functions_and_reassigned_bindings_keep_snapshots() {
    let a = checked(&[(
        "main.ubi",
        r#"
fn add(n: int) -> fn(int,) -> int { (x: int) => n + x }
fn nested(a: int) -> fn(int) -> fn(int) -> int {
  (b: int) => (c: int) => a + b + c
}
fn remember() -> fn(int) -> int {
  let mut f: fn(int) -> int = add(1);
  let saved: fn(int) -> int = (x: int) => f(x);
  f = add(100);
  saved
}
fn localMutation() -> fn() -> int {
  () => { let mut x = 1; x = x + 4; x }
}
export fn main() -> int { nested(10)(20)(3) + remember()(4) + localMutation()() }
"#,
    )]);
    assert_eq!(run(&a, "main", vec![]), Ok(Value::Int(43)));
    node(&a, "assert.equal(m.main(),43); assert.equal(m.main(),43);");
}

#[test]
fn callee_then_arguments_evaluate_once_in_source_order() {
    let a = checked(&[(
        "main.ubi",
        r#"
export fn main() -> int {
  let mut n = 0;
  let value = { n = n + 1; (a: int, b: int) => a * 10 + b }
    ({ n = n + 1; n }, { n = n + 1; n });
  value * 100 + n
}
"#,
    )]);
    assert_eq!(run(&a, "main", vec![]), Ok(Value::Int(2303)));
    node(&a, "assert.equal(m.main(),2303);");
}

#[test]
fn every_supported_aggregate_stores_and_recovers_typed_callbacks() {
    let a = checked(&[(
        "main.ubi",
        r#"
record Box<T> { value: T }
enum Choice<T> { Empty, Present(T) }
fn increment(x: int) -> int { x + 1 }
export fn main() -> int {
  let xs: List<fn(int) -> int> = [increment];
  let option: Option<fn(int) -> int> = Option.Some(increment);
  let result: Result<fn(int) -> int,string> = Result.Ok(increment);
  let entries: Map<string,fn(int) -> int> = mapSet(mapEmpty(),"f",increment);
  let boxed: Box<fn(int) -> int> = Box { value: increment };
  let choice: Choice<fn(int) -> int> = Choice.Present(increment);
  let a = match (xs[0]) { Option.Some(f) => f(1), Option.None => 0 };
  let b = match (option) { Option.Some(f) => f(2), Option.None => 0 };
  let c = match (result) { Result.Ok(f) => f(3), Result.Err(e) => 0 };
  let d = match (mapGet(entries,"f")) { Option.Some(f) => f(4), Option.None => 0 };
  let e = boxed.value(5);
  let f = match (choice) { Choice.Present(f) => f(6), Choice.Empty => 0 };
  a + b + c + d + e + f
}
"#,
    )]);
    assert_eq!(run(&a, "main", vec![]), Ok(Value::Int(27)));
    node(&a, "assert.equal(m.main(),27);");
}

#[test]
fn inferred_aggregate_and_generic_wrappers_accept_function_values() {
    let a = checked(&[(
        "main.ubi",
        r#"
record Box<T> { value: T }
enum Choice<T> { Present(T) }
fn list<T>(x: T) -> List<T> { [x] }
fn option<T>(x: T) -> Option<T> { Option.Some(x) }
fn boxed<T>(x: T) -> Box<T> { Box { value: x } }
fn choice<T>(x: T) -> Choice<T> { Choice.Present(x) }
export fn main() -> int {
  let xs = [(x: int) => x + 1];
  let some = Option.Some((x: int) => x + 2);
  let a = match (xs[0]) { Option.Some(f) => f(1), Option.None => 0 };
  let b = match (some) { Option.Some(f) => f(2), Option.None => 0 };
  let c = match (list((x: int) => x + 3)[0]) { Option.Some(f) => f(3), Option.None => 0 };
  let d = match (option((x: int) => x + 4)) { Option.Some(f) => f(4), Option.None => 0 };
  let e = boxed((x: int) => x + 5).value(5);
  let f = match (choice((x: int) => x + 6)) { Choice.Present(f) => f(6) };
  a + b + c + d + e + f
}
"#,
    )]);
    assert_eq!(run(&a, "main", vec![]), Ok(Value::Int(42)));
    node(&a, "assert.equal(m.main(),42);");
}

#[test]
fn generic_inference_uses_later_arguments_and_nested_expected_signatures() {
    let a = checked(&[(
        "main.ubi",
        r#"
record Box<T> { value: T }
fn identity<T>(x: T) -> T { x }
fn apply<T>(f: fn(T) -> T, x: T) -> T { f(x) }
fn keep<T>(x: T) -> fn() -> T { () => x }
fn unwrap<T>(box: Box<fn(T) -> T>, value: T) -> T { box.value(value) }
export fn main() -> int {
  let named: fn(int) -> int = identity;
  apply(identity,40) + unwrap(Box { value: identity },1) + keep(named)()(1)
}
"#,
    )]);
    assert_eq!(run(&a, "main", vec![]), Ok(Value::Int(42)));
    node(&a, "assert.equal(m.main(),42);");
}

#[test]
fn module_exports_carry_functions_and_enforce_nested_public_type_privacy() {
    let a = checked(&[
        (
            "callbacks.ubi",
            r#"
export record Box { value: fn(int) -> int }
export fn make(n: int) -> fn(int) -> int { (x: int) => x + n }
export fn apply(f: fn(int) -> int, x: int) -> int { f(x) }
export fn box(n: int) -> Box { Box { value: make(n) } }
"#,
        ),
        (
            "main.ubi",
            r#"
import { Box, make, apply, box } from "./callbacks.ubi";
export fn main() -> int { apply(make(2),20) + box(3).value(17) }
"#,
        ),
    ]);
    assert_eq!(run(&a, "main", vec![]), Ok(Value::Int(42)));
    node(&a, "assert.equal(m.main(),42); const c=await import('./callbacks.mjs'); assert.throws(()=>c.make(1),TypeError); assert.throws(()=>c.apply(x=>x,1),TypeError); assert.throws(()=>c.box(1),TypeError);");
    for source in [
        "record Hidden {} export fn f(x: fn(Hidden) -> int) -> int { 0 }",
        "record Hidden {} export fn f(x: fn(int) -> Hidden) -> int { 0 }",
        "record Hidden {} export record R { callback: Option<fn() -> Hidden> }",
        "record Hidden {} export enum E { A(fn(fn(Hidden) -> int) -> int) }",
    ] {
        rejected(&[("main.ubi", source)], "UBI0013");
    }
    rejected(&[
        ("a.ubi", "export record R {} export fn make() -> fn(R) -> int { (x: R) => 1 }"),
        ("b.ubi", "export record R {} export fn take(f: fn(R) -> int) -> int { f(R {}) }"),
        ("main.ubi", "import { make } from \"./a.ubi\"; import { take } from \"./b.ubi\"; export fn main() -> int { take(make()) }"),
    ], "UBI0020");
}

#[test]
fn host_rejects_declared_function_outputs_before_evaluating_faulting_bodies() {
    let a = checked(&[(
        "main.ubi",
        r#"
export record Box { value: List<fn() -> int> }
export enum Choice { Empty, Callback(fn() -> int) }
export fn direct() -> fn() -> int { let bad = 1 / 0; () => bad }
export fn list() -> List<fn() -> int> { let bad = 1 / 0; [] }
export fn option() -> Option<fn() -> int> { let bad = 1 / 0; Option.None }
export fn boxed() -> Box { let bad = 1 / 0; Box { value: [] } }
export fn choice() -> Choice { let bad = 1 / 0; Choice.Empty }
export fn entries() -> Map<string,fn() -> int> { let bad = 1 / 0; mapEmpty() }
export async fn asyncList() -> List<fn() -> int> { let bad = 1 / 0; [] }
export record Marker<T> { n: int }
export fn phantom() -> Marker<fn() -> int> { let bad = 1 / 0; Marker { n: 1 } }
fn internal() -> fn() -> int { () => 42 }
export fn ordinary() -> int { internal()() }
"#,
    )]);
    for name in [
        "direct",
        "list",
        "option",
        "boxed",
        "choice",
        "entries",
        "asyncList",
        "phantom",
    ] {
        assert_eq!(
            run(&a, name, vec![]),
            Err(InvocationError::InvalidArguments),
            "{name}"
        );
    }
    assert_eq!(run(&a, "ordinary", vec![]), Ok(Value::Int(42)));
    node(&a, "for(const name of ['direct','list','option','boxed','choice','entries','phantom']) assert.throws(()=>m[name](),TypeError); await assert.rejects(m.asyncList(),TypeError); assert.equal(m.ordinary(),42);");
}

#[test]
fn host_rejects_empty_function_inputs_by_declared_type() {
    let a = checked(&[(
        "main.ubi",
        r#"
export record Box { value: List<fn() -> int> }
export enum Choice { Empty, Callback(fn() -> int) }
export fn list(x: List<fn() -> int>) -> int { 1 / 0 }
export fn option(x: Option<fn() -> int>) -> int { 1 / 0 }
export fn choice(x: Choice) -> int { 1 / 0 }
export fn boxed(x: Box) -> int { 1 / 0 }
export fn entries(x: Map<string,fn() -> int>) -> int { 1 / 0 }
"#,
    )]);
    for name in ["list", "option", "choice", "boxed", "entries"] {
        let ty = a.signatures[&("main.ubi".into(), name.into())].parameters[0];
        let value = match ty {
            ValueType::List(type_id) => Value::List {
                type_id,
                elements: Rc::new(vec![]),
            },
            ValueType::Option(type_id) => Value::Option {
                type_id,
                value: None,
            },
            ValueType::Enum(type_id) => Value::Enum {
                type_id,
                tag: Rc::new("Empty".into()),
                values: Rc::new(vec![]),
            },
            ValueType::Map(type_id) => Value::Map {
                type_id,
                entries: Rc::new(vec![]),
            },
            ValueType::Record(type_id) => {
                let ValueType::List(list_id) = a.records[type_id].fields["value"] else {
                    panic!("expected list")
                };
                Value::Record {
                    type_id,
                    fields: Rc::new(
                        [(
                            "value".into(),
                            Value::List {
                                type_id: list_id,
                                elements: Rc::new(vec![]),
                            },
                        )]
                        .into_iter()
                        .collect(),
                    ),
                }
            }
            _ => panic!("unexpected parameter type"),
        };
        assert_eq!(
            run(&a, name, vec![value]),
            Err(InvocationError::InvalidArguments),
            "{name}"
        );
    }
    node(&a, "for(const [name,value] of [['list',[]],['option',{tag:'None'}],['choice',{tag:'Empty',values:[]}],['boxed',{value:[]}],['entries',[]]]) assert.throws(()=>m[name](JSON.stringify(value)),TypeError);");
}

#[test]
fn indirect_returned_and_imported_callbacks_share_runtime_limits() {
    let a = checked(&[
        (
            "callbacks.ubi",
            r#"
export fn recurse(n: int) -> int {
  if (n == 0) { 0 } else {
    match ([recurse][0]) { Option.Some(f) => f(n - 1), Option.None => 99 }
  }
}
export fn identity(x: int) -> int { x }
export fn make() -> fn(int) -> int { identity }
"#,
        ),
        (
            "main.ubi",
            r#"
import { recurse, make } from "./callbacks.ubi";
fn apply(f: fn(int) -> int, x: int) -> int { f(x) }
export fn withinFrames() -> int { apply(recurse,3) }
export fn overFrames() -> int { apply(recurse,40) }
export fn overSteps() -> int {
  let callbacks: List<fn(int) -> int> = [make()];
  let mut total = 0;
  for (n in range(0,200000)) {
    total = total + match (callbacks[0]) { Option.Some(f) => f(1), Option.None => 0 };
  }
  total
}
"#,
        ),
    ]);
    assert_eq!(run(&a, "withinFrames", vec![]), Ok(Value::Int(0)));
    for name in ["overFrames", "overSteps"] {
        let Err(InvocationError::Runtime(fault)) = run(&a, name, vec![]) else {
            panic!("expected runtime budget failure: {name}")
        };
        assert_eq!(fault.code, "UBI-R0005");
    }
    node(&a, "assert.equal(m.withinFrames(),0); for(const name of ['overFrames','overSteps']) assert.throws(()=>m[name](),e=>e.code==='UBI-R0005');");
}

#[test]
fn invariant_signatures_arity_and_function_containing_equality_are_rejected() {
    for source in [
        "fn f() { let x: fn(int) -> int = (x: bool) => 1; }",
        "fn f() { let x: fn(int) -> int = (x: int) => true; }",
        "fn f() { let x: fn(int,int) -> int = (x: int) => x; }",
        "fn id<T>(x: T) -> T { x } fn f() { let x = id; }",
        "fn f(g: fn() -> int) -> bool { g == g }",
        "fn f() -> bool { let xs: List<fn() -> int> = []; xs == xs }",
        "fn f() -> bool { let x: Option<fn() -> int> = Option.None; x == x }",
        "enum E { Empty, F(fn() -> int) } fn f() -> bool { E.Empty == E.Empty }",
        "record R { value: List<fn() -> int> } fn f() -> bool { R { value: [] } == R { value: [] } }",
        "fn f() -> bool { let x: Map<string,fn() -> int> = mapEmpty(); x == x }",
        "fn f(x: Map<fn() -> int,int>) {}",
    ] {
        rejected(&[("main.ubi", source)], "UBI0020");
    }
    for source in [
        "fn f(g: fn(int) -> int) -> int { g() }",
        "fn f(g: fn(int) -> int) -> int { g(1,2) }",
    ] {
        rejected(&[("main.ubi", source)], "UBI0023");
    }
}

#[test]
fn phantom_record_function_argument_is_not_equatable() {
    rejected(&[("main.ubi", "record Phantom<T> { n: int } fn f() -> bool { let x: Phantom<fn() -> int> = Phantom { n: 1 }; x == x }")], "UBI0020");
}

#[test]
fn phantom_enum_function_argument_is_not_equatable() {
    rejected(&[("main.ubi", "enum Phantom<T> { Empty } fn f() -> bool { let x: Phantom<fn() -> int> = Phantom.Empty; x == x }")], "UBI0020");
}

#[test]
fn malformed_function_types_use_parser_diagnostics() {
    for source in [
        "fn f(x: fn(int)) {}",
        "fn f(x: fn(int) ->) {}",
        "fn f(x: fn(int, -> int) {}",
        "fn f(x: fn int -> int) {}",
    ] {
        let a = analysis(&[("main.ubi", source)]);
        assert!(
            a.diagnostics.iter().any(|d| d.code.starts_with("UBI000")),
            "{source}: {:#?}",
            a.diagnostics
        );
        assert!(crate::ir::lower(&a).is_err());
    }
}

#[test]
fn typed_function_values_cannot_smuggle_effects_async_or_capabilities() {
    for source in [
        "fn f(g: fn(Console) -> int) {}",
        "fn f(g: fn() -> Storage) {}",
        "fn f(g: fn(List<fn(Http) -> int>) -> int) {}",
        "record R<T> { value: T } fn f(g: fn(R<Console>) -> int) {}",
        "fn generic<T>(f: fn(T) -> int, x: T) -> int { f(x) } fn plain(c: Console) -> int { 1 } fn bad(c: Console) -> int { generic(plain,c) }",
        "fn write(c: Console) -> int effects(console) { consoleLog(c,\"x\"); 1 } fn bad(c: Console) { let f: fn(Console) -> int = write; }",
        "fn write(c: Console) -> int effects(console) { consoleLog(c,\"x\"); 1 } fn bad(c: Console) { let f: fn() -> int = () => write(c); }",
        "fn declared() -> int effects(console) { 1 } fn bad() { let f: fn() -> int = declared; }",
        "fn declared() -> int effects(console) { 1 } fn bad() -> fn() -> int { if (false) { declared } else { () => 1 } }",
    ] {
        rejected(&[("main.ubi", source)], "UBI0040");
    }
    for source in [
        "async fn value() -> int { 1 } fn bad() { let f: fn() -> int = value; }",
        "async fn value() -> int { 1 } fn take(f: fn() -> int) -> int { f() } fn bad() -> int { take(value) }",
        "async fn value() -> int { 1 } fn bad() -> fn() -> int { if (false) { value } else { () => 1 } }",
    ] {
        rejected(&[("main.ubi", source)], "UBI0041");
    }
}
