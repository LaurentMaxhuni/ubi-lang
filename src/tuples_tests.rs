//! Independent conformance for SPEC section 24.
use crate::analyzer::{Analysis, TypeInfo, ValueType};
use crate::formatter::format_source;
use crate::generics_tests::{analysis, checked, node, rejected, run};
use crate::interpreter::{InvocationError, Value};
use crate::lexer::lex;
use std::rc::Rc;

#[derive(Debug)]
enum Expected {
    Int(i32),
    String(&'static str),
    Unit,
    Tuple(Vec<Expected>),
}

fn assert_value(actual: Result<Value, InvocationError>, expected: Expected) {
    fn matches(actual: &Value, expected: &Expected) {
        match (actual, expected) {
            (Value::Int(actual), Expected::Int(expected)) => assert_eq!(actual, expected),
            (Value::String(actual), Expected::String(expected)) => {
                assert_eq!(actual.as_str(), *expected)
            }
            (Value::Unit, Expected::Unit) => {}
            (Value::Tuple { elements, .. }, Expected::Tuple(expected)) => {
                assert_eq!(elements.len(), expected.len());
                for (actual, expected) in elements.iter().zip(expected) {
                    matches(actual, expected);
                }
            }
            _ => panic!("value {actual:?} does not match {expected:?}"),
        }
    }

    let actual = actual.expect("expected successful invocation");
    matches(&actual, &expected);
}

fn tuple_elements(a: &Analysis, ty: ValueType) -> &[ValueType] {
    let ValueType::Tuple(type_id) = ty else {
        panic!("tuple type expected")
    };
    let TypeInfo::Tuple(elements) = &a.types[type_id] else {
        panic!("interned tuple type expected")
    };
    elements
}

fn value_depth(value: &Value) -> usize {
    match value {
        Value::Tuple { elements, .. } => 1 + elements.iter().map(value_depth).max().unwrap_or(0),
        Value::Enum { values, .. } => 1 + values.iter().map(value_depth).max().unwrap_or(0),
        _ => 0,
    }
}

fn tuple_value(_a: &Analysis, ty: ValueType, elements: Vec<Value>) -> Value {
    let ValueType::Tuple(type_id) = ty else {
        panic!("tuple type expected")
    };
    Value::Tuple {
        type_id,
        elements: Rc::new(elements),
    }
}

fn tuple_parameter(a: &Analysis, function: &str, index: usize) -> ValueType {
    a.signatures[&("main.ubi".into(), function.into())].parameters[index]
}

fn fault(a: &Analysis, name: &str, code: &str) {
    let Err(InvocationError::Runtime(fault)) = run(a, name, vec![]) else {
        panic!("expected {code} from {name}");
    };
    assert_eq!(fault.code, code, "{name}");
}

#[test]
fn singleton_nested_grouped_and_unit_values_have_fixed_shapes() {
    let a = checked(&[(
        "main.ubi",
        r#"
export fn product() -> (int, (string,), unit) {
  let singleton: (int,) = (42,);
  let nested: ((int, int), (int,)) = ((2, 3), (4,));
  let ((a, b), (c,)) = nested;
  let grouped: int = (a + b + c);
  let empty: () = ();
  let unitText = match (empty) { () => "unit" };
  let (only,) = singleton;
  (only, (unitText + toString(grouped),), empty)
}
"#,
    )]);
    assert_value(
        run(&a, "product", vec![]),
        Expected::Tuple(vec![
            Expected::Int(42),
            Expected::Tuple(vec![Expected::String("unit9")]),
            Expected::Unit,
        ]),
    );
    node(
        &a,
        "const value=m.product(); assert.deepEqual(value,[42,['unit9'],undefined]); assert.ok(Object.isFrozen(value)); assert.ok(Object.isFrozen(value[1]));",
    );
}

#[test]
fn destructuring_initializes_once_shadows_simultaneously_and_captures_values() {
    let a = checked(&[(
        "main.ubi",
        r#"
export fn initializedOnce() -> int {
  let mut n = 0;
  let (first, (_, last)): (int, (int, int)) = {
    n = n + 1;
    (n, ({ n = n + 1; n }, { n = n + 1; n }))
  };
  n * 100 + first * 10 + last
}
export fn simultaneousShadow() -> int {
  let (first, last) = (2, 3);
  let saved = {
    let (first, last) = (last, first);
    () => first * 10 + last
  };
  saved()
}
export fn reassignedSourceSnapshot() -> int {
  let mut pair = (2, 3);
  let saved = { let (first, last) = pair; () => first * 10 + last };
  pair = (8, 9);
  saved()
}
"#,
    )]);
    for (name, expected) in [
        ("initializedOnce", 313),
        ("simultaneousShadow", 32),
        ("reassignedSourceSnapshot", 23),
    ] {
        assert_eq!(run(&a, name, vec![]), Ok(Value::Int(expected)), "{name}");
    }
    node(
        &a,
        "assert.equal(m.initializedOnce(),313); assert.equal(m.simultaneousShadow(),32); assert.equal(m.reassignedSourceSnapshot(),23);",
    );
}

#[test]
fn tuple_elements_run_left_to_right_and_stop_on_return_propagation_or_fault() {
    let a = checked(&[(
        "main.ubi",
        r#"
fn fail(value: int) -> Result<int, string> { Result.Err(toString(value)) }
fn attempt() -> Result<(int, int, int), string> {
  let mut n = 0;
  Result.Ok(({ n = n + 1; n }, fail(n)?, { n = n + 1; n }))
}
export fn order() -> int {
  let mut n = 0;
  let (a, b, c) = (
    { n = n + 1; n },
    { n = n + 1; n },
    { n = n + 1; n },
  );
  n * 100 + a * 10 + b * 2 + c
}
export fn earlyReturn() -> int { let (ignored, _) = ({ return 17; 0 }, 1 / 0); ignored }
export fn firstFault() -> (int, int) { (1 / 0, 2147483647 + 1) }
export fn propagationStops() -> int {
  match (attempt()) {
    Result.Ok(_) => 0,
    Result.Err(message) => if (message == "1") { 1 } else { 2 },
  }
}
"#,
    )]);
    assert_eq!(run(&a, "order", vec![]), Ok(Value::Int(317)));
    assert_eq!(run(&a, "earlyReturn", vec![]), Ok(Value::Int(17)));
    assert_eq!(run(&a, "propagationStops", vec![]), Ok(Value::Int(1)));
    fault(&a, "firstFault", "UBI-R0002");
    node(
        &a,
        "assert.equal(m.order(),317); assert.equal(m.earlyReturn(),17); assert.equal(m.propagationStops(),1); assert.throws(()=>m.firstFault(),e=>e.code==='UBI-R0002');",
    );
}

#[test]
fn structural_types_expected_contexts_and_generic_substitution_flow_through_tuples() {
    let a = checked(&[(
        "main.ubi",
        r#"
fn identity<T>(x: T) -> T { x }
fn empty<T>() -> List<T> { [] }
fn apply<T>(f: fn(T) -> T, x: T) -> T { f(x) }
fn increment(x: int) -> int { x + 1 }
export fn main() -> int {
  let emptyPair: (List<int>, List<int>) = (empty(), empty());
  let (left, right) = emptyPair;
  let (first, (second,)) = identity((20, (21,)));
  let functions: (fn(int) -> int, fn(int) -> int) = (identity, increment);
  let (id, inc) = functions;
  length(left) + length(right) + first + second + apply(id, 0) + inc(0)
}
"#,
    )]);
    assert_eq!(run(&a, "main", vec![]), Ok(Value::Int(42)));
    node(&a, "assert.equal(m.main(),42);");
}

#[test]
fn imported_tuple_signatures_work_and_private_types_stay_private() {
    let a = checked(&[
        (
            "pairs.ubi",
            "export fn make(x: int) -> (int, (int,)) { (x, (x + 1,)) } export fn sum(x: (int, (int,))) -> int { let (a, (b,)) = x; a + b }",
        ),
        (
            "main.ubi",
            "import { make, sum } from \"./pairs.ubi\"; export fn main() -> int { sum(make(20)) + sum((0, (1,))) }",
        ),
    ]);
    assert_eq!(run(&a, "main", vec![]), Ok(Value::Int(42)));
    node(&a, "assert.equal(m.main(),42);");
    rejected(
        &[(
            "main.ubi",
            "record Hidden {} export fn leak() -> (Hidden, unit) { (Hidden {}, ()) }",
        )],
        "UBI0013",
    );
    rejected(
        &[(
            "main.ubi",
            "record Hidden {} export record Public { value: (Hidden,) }",
        )],
        "UBI0013",
    );
}

#[test]
fn tuple_matches_keep_product_correlation_and_check_nested_payloads_and_guards() {
    let a = checked(&[(
        "main.ubi",
        r#"
enum Payload { Empty, Pair((bool, bool)) }
fn classify(pair: (bool, bool)) -> int {
  match (pair) {
    (true, true) => 11,
    (true, false) => 12,
    (false, true) => 21,
    (false, false) => 22,
  }
}
fn decode(payload: Payload) -> int {
  match (payload) {
    Payload.Empty => 0,
    Payload.Pair((true, true)) => 1,
    Payload.Pair((true, false)) => 2,
    Payload.Pair((false, true)) => 3,
    Payload.Pair((false, false)) => 4,
  }
}
export fn matrix() -> int {
  classify((true, true)) + classify((true, false)) * 10
    + classify((false, true)) * 100 + classify((false, false)) * 1000
}
export fn nestedPayload() -> int { decode(Payload.Pair((false, true))) }
export fn unitPattern() -> int { match (()) { () => 7 } }
export fn guardOrder() -> int {
  let mut visits = 0;
  let result = match ((true, false)) {
    (true, true) => 1,
    (true, false) if ({ visits = visits + 1; false }) => 2,
    (true, false) if ({ visits = visits + 1; true }) => 3,
    (true, false) => 5,
    (false, _) => 4,
  };
  visits * 100 + result
}
"#,
    )]);
    assert_eq!(run(&a, "matrix", vec![]), Ok(Value::Int(24_231)));
    assert_eq!(run(&a, "nestedPayload", vec![]), Ok(Value::Int(3)));
    assert_eq!(run(&a, "unitPattern", vec![]), Ok(Value::Int(7)));
    assert_eq!(run(&a, "guardOrder", vec![]), Ok(Value::Int(203)));
    node(&a, "assert.equal(m.matrix(),24231); assert.equal(m.nestedPayload(),3); assert.equal(m.unitPattern(),7); assert.equal(m.guardOrder(),203);");

    for source in [
        "fn f(pair: (bool, bool)) -> int { match (pair) { (true, true) => 1, (false, false) => 2 } }",
        "fn f(pair: (bool, bool)) -> int { match (pair) { (true, _) if (true) => 1, (false, _) => 2 } }",
    ] {
        rejected(&[("main.ubi", source)], "UBI0020");
    }
}

#[test]
fn tuple_equality_is_ordered_and_requires_equatable_elements() {
    let a = checked(&[(
        "main.ubi",
        r#"
export fn equality() -> bool {
  let first = (1, (2,));
  let same = (1, (2,));
  first == same && (1, 2) != (2, 1) && ((1,), ()) == ((1,), ())
}

export fn nanEquality() -> bool {
  let nan = 0.0 / 0.0;
  (nan, ()) == (nan, ())
}
"#,
    )]);
    assert_eq!(run(&a, "equality", vec![]), Ok(Value::Bool(true)));
    assert_eq!(run(&a, "nanEquality", vec![]), Ok(Value::Bool(false)));
    node(
        &a,
        "assert.equal(m.equality(),true); assert.equal(m.nanEquality(),false);",
    );

    for source in [
        "fn f(g: fn() -> int) -> bool { (g, ()) == (g, ()) }",
        "fn f() -> bool { let xs: List<(fn() -> int, unit)> = []; xs == xs }",
        "enum E { Empty, Callback((fn() -> int, unit)) } fn f() -> bool { E.Empty == E.Empty }",
        "record Phantom<T> { n: int } fn f() -> bool { let x: Phantom<(fn() -> int, unit)> = Phantom { n: 1 }; x == x }",
    ] {
        rejected(&[("main.ubi", source)], "UBI0020");
    }
}

#[test]
fn equality_memoizes_shared_tuple_and_list_dags_without_short_circuiting_nan() {
    let mut source = String::from(
        "fn pair<T>(value: T) -> (T, T) { (value, value) } export fn sameGraph() -> bool { let a1 = pair(1);",
    );
    for depth in 2..=27 {
        source.push_str(&format!(" let a{depth} = pair(a{});", depth - 1));
    }
    source.push_str(" a27 == a27 } export fn distinctGraphs() -> bool { let left1 = pair(1);");
    for depth in 2..=27 {
        source.push_str(&format!(" let left{depth} = pair(left{});", depth - 1));
    }
    source.push_str(" let right1 = pair(1);");
    for depth in 2..=27 {
        source.push_str(&format!(" let right{depth} = pair(right{});", depth - 1));
    }
    source.push_str(" left27 == right27 } export fn sharedNaN() -> bool { let nan = 0.0 / 0.0; let a1 = pair(nan);");
    for depth in 2..=27 {
        source.push_str(&format!(" let a{depth} = pair(a{});", depth - 1));
    }
    source.push_str(" a27 == a27 } fn dup<T>(value: T) -> List<T> { [value, value] } export fn sameListGraph() -> bool { let a1 = dup(1);");
    for depth in 2..=27 {
        source.push_str(&format!(" let a{depth} = dup(a{});", depth - 1));
    }
    source.push_str(" a27 == a27 } export fn distinctListGraphs() -> bool { let left1 = dup(1);");
    for depth in 2..=27 {
        source.push_str(&format!(" let left{depth} = dup(left{});", depth - 1));
    }
    source.push_str(" let right1 = dup(1);");
    for depth in 2..=27 {
        source.push_str(&format!(" let right{depth} = dup(right{});", depth - 1));
    }
    source.push_str(" left27 == right27 } export fn sharedListNaN() -> bool { let nan = 0.0 / 0.0; let a1 = dup(nan);");
    for depth in 2..=27 {
        source.push_str(&format!(" let a{depth} = dup(a{});", depth - 1));
    }
    source.push_str(" a27 == a27 }");

    let a = checked(&[("main.ubi", &source)]);
    assert_eq!(run(&a, "sameGraph", vec![]), Ok(Value::Bool(true)));
    assert_eq!(run(&a, "distinctGraphs", vec![]), Ok(Value::Bool(true)));
    assert_eq!(run(&a, "sharedNaN", vec![]), Ok(Value::Bool(false)));
    assert_eq!(run(&a, "sameListGraph", vec![]), Ok(Value::Bool(true)));
    assert_eq!(run(&a, "distinctListGraphs", vec![]), Ok(Value::Bool(true)));
    assert_eq!(run(&a, "sharedListNaN", vec![]), Ok(Value::Bool(false)));
    node(
        &a,
        "assert.equal(m.sameGraph(),true); assert.equal(m.distinctGraphs(),true); assert.equal(m.sharedNaN(),false); assert.equal(m.sameListGraph(),true); assert.equal(m.distinctListGraphs(),true); assert.equal(m.sharedListNaN(),false);",
    );
}

#[test]
fn destructuring_diagnostics_reject_duplicates_refutable_shapes_and_type_mismatches() {
    for source in [
        "fn f() { let (x, (x, _)) = (1, (2, 3)); }",
        "fn f() { let (x, y) = (1, 2); let (x, z) = (3, 4); }",
    ] {
        rejected(&[("main.ubi", source)], "UBI0011");
    }
    for source in [
        "fn f() { let (x, _) = (1, 2, 3); }",
        "fn f() { let (x, _) = 1; }",
        "fn f() { let (1, x) = (1, 2); }",
    ] {
        rejected(&[("main.ubi", source)], "UBI0031");
    }
    for source in [
        "fn f() { let x: (int, string) = (1, 2); }",
        "fn f() { let x: (int,) = (1, 2); }",
        "fn f() { let x: (int, int) = (1,); }",
        "fn f() { let x: (int, int) = (1, true); }",
        "fn f() { let x = (1,); let y: (int,) = (1); }",
    ] {
        rejected(&[("main.ubi", source)], "UBI0020");
    }
}

#[test]
fn capabilities_and_ranges_cannot_be_stored_in_tuples() {
    rejected(
        &[("main.ubi", "fn f(c: Console) { let value = (c, ()); }")],
        "UBI0040",
    );
    rejected(
        &[("main.ubi", "fn f() { let value = ((range(0, 1),), ()); }")],
        "UBI0020",
    );
    rejected(
        &[("main.ubi", "fn f(g: fn((Console,)) -> int) {}")],
        "UBI0040",
    );
}

#[test]
fn public_host_tuples_validate_exact_arity_types_and_frozen_json_arrays() {
    let a = checked(&[(
        "main.ubi",
        r#"
export fn main(value: (int, (string,))) -> int {
  let (number, (text,)) = value;
  number + length(text)
}
export fn echo(value: (int, (string,))) -> (int, (string,)) { value }
export fn echoUnit(value: (unit, int)) -> (unit, int) { value }
"#,
    )]);
    let outer_type = tuple_parameter(&a, "main", 0);
    let inner_type = tuple_elements(&a, outer_type)[1];
    let valid = tuple_value(
        &a,
        outer_type,
        vec![
            Value::Int(39),
            tuple_value(&a, inner_type, vec![Value::String("abc".into())]),
        ],
    );
    assert_eq!(run(&a, "main", vec![valid.clone()]), Ok(Value::Int(42)));
    for invalid in [
        tuple_value(&a, outer_type, vec![Value::Int(39)]),
        tuple_value(
            &a,
            outer_type,
            vec![Value::Int(39), Value::String("abc".into())],
        ),
        tuple_value(
            &a,
            outer_type,
            vec![
                Value::Int(39),
                tuple_value(
                    &a,
                    inner_type,
                    vec![Value::String("a".into()), Value::String("b".into())],
                ),
            ],
        ),
        tuple_value(
            &a,
            outer_type,
            vec![
                Value::Bool(true),
                tuple_value(&a, inner_type, vec![Value::String("abc".into())]),
            ],
        ),
    ] {
        assert_eq!(
            run(&a, "main", vec![invalid]),
            Err(InvocationError::InvalidArguments)
        );
    }
    node(
        &a,
        r#"
assert.equal(m.main('[39,["abc"]]'),42);
assert.throws(()=>m.main([39,['abc']]),TypeError);
const copied=JSON.parse('[39,["abc"]]');
const echo=m.echo(JSON.stringify(copied));
assert.deepEqual(echo,[39,['abc']]);
assert.ok(Object.isFrozen(echo)); assert.ok(Object.isFrozen(echo[1]));
assert.ok(!Object.isFrozen(copied)); assert.ok(!Object.isFrozen(copied[1]));
assert.deepEqual(m.echoUnit('[null,7]'),[undefined,7]);
for(const bad of ['[39]','[39,"abc"]','[39,["a","b"]]','[true,["abc"]]','[39,["abc",null]]','null','{}']) assert.throws(()=>m.main(bad),TypeError);
"#,
    );
}

#[test]
fn public_hosts_reject_function_containing_tuple_types_before_execution() {
    let a = checked(&[(
        "main.ubi",
        r#"
fn callback() -> int { 42 }
export enum Choice { Empty, Callback(fn() -> int) }
export fn direct() -> (fn() -> int, unit) { let bad = 1 / 0; (callback, ()) }
export fn nestedEmpty() -> ((List<fn() -> int>,), unit) { let bad = 1 / 0; (([],), ()) }
export fn none() -> (Option<fn() -> int>, unit) { let bad = 1 / 0; (Option.None, ()) }
export fn unusedVariant() -> (Choice, unit) { let bad = 1 / 0; (Choice.Empty, ()) }
export fn nestedCallback() -> ((fn() -> int,), unit) { let bad = 1 / 0; ((callback,), ()) }
export fn consume(value: (List<fn() -> int>, unit)) -> int { let bad = 1 / 0; 0 }
"#,
    )]);
    for name in [
        "direct",
        "nestedEmpty",
        "none",
        "unusedVariant",
        "nestedCallback",
    ] {
        assert_eq!(
            run(&a, name, vec![]),
            Err(InvocationError::InvalidArguments),
            "{name}"
        );
    }
    let consume_type = tuple_parameter(&a, "consume", 0);
    let ValueType::List(callback_id) = tuple_elements(&a, consume_type)[0] else {
        panic!("list of callbacks expected")
    };
    let empty_callbacks = tuple_value(
        &a,
        consume_type,
        vec![
            Value::List {
                type_id: callback_id,
                elements: Rc::new(vec![]),
            },
            Value::Unit,
        ],
    );
    assert_eq!(
        run(&a, "consume", vec![empty_callbacks]),
        Err(InvocationError::InvalidArguments)
    );
    node(
        &a,
        "for(const name of ['direct','nestedEmpty','none','unusedVariant','nestedCallback']) assert.throws(()=>m[name](),TypeError); assert.throws(()=>m.consume('[[],null]'),TypeError);",
    );
}

#[test]
fn functions_stored_in_tuples_keep_the_caller_runtime_budget() {
    let a = checked(&[(
        "main.ubi",
        r#"
fn recurse(n: int) -> int {
  if (n == 0) { 0 } else {
    let (f, next): (fn(int) -> int, int) = (recurse, n - 1);
    f(next)
  }
}
export fn withinFrames() -> int { recurse(3) }
export fn overFrames() -> int { recurse(40) }
"#,
    )]);
    assert_eq!(run(&a, "withinFrames", vec![]), Ok(Value::Int(0)));
    fault(&a, "overFrames", "UBI-R0005");
    node(&a, "assert.equal(m.withinFrames(),0); assert.throws(()=>m.overFrames(),e=>e.code==='UBI-R0005');");
}

#[test]
fn generic_pair_chains_select_one_shared_branch_without_exponential_copying() {
    let mut source = String::from(
        "fn pair<T>(value: T) -> (T, T) { (value, value) } fn discard<T>(value: T) -> unit { () } fn build<T>(seed: T) -> int { let a1 = pair(seed);",
    );
    for depth in 2..=28 {
        source.push_str(&format!(" let a{depth} = pair(a{});", depth - 1));
    }
    let mut selected = "a28".to_owned();
    for depth in (2..=28).rev() {
        let binding = format!("d{depth}");
        source.push_str(&format!(" let ({binding}, _) = {selected};"));
        selected = binding;
    }
    source.push_str(&format!(
        " let (result, _) = {selected}; discard(result); 1 }} export fn main() -> int {{ build(1) }}"
    ));

    let a = checked(&[("main.ubi", &source)]);
    assert_eq!(run(&a, "main", vec![]), Ok(Value::Int(1)));
    node(&a, "assert.equal(m.main(),1);");
}

#[test]
fn nested_tuple_depth_uses_existing_aggregate_limits() {
    let a = checked(&[(
        "main.ubi",
        r#"
export enum Chain { End, Next((Chain,)) }
fn grow(n: int, tail: Chain) -> Chain {
  if (n == 0) { tail } else { grow(n - 1, Chain.Next((tail,))) }
}
export fn make32() -> (Chain,) { (grow(15, Chain.End),) }
export fn make33() -> Chain { grow(16, Chain.End) }
export fn host32(value: (Chain,)) -> int { 32 }
export fn host33(value: Chain) -> int { 1 / 0 }
"#,
    )]);
    let within = run(&a, "make32", vec![]).unwrap();
    assert_eq!(value_depth(&within), 32);
    assert_eq!(run(&a, "host32", vec![within]), Ok(Value::Int(32)));
    let ValueType::Enum(chain_id) = tuple_parameter(&a, "host33", 0) else {
        panic!("recursive enum type expected")
    };
    let TypeInfo::Enum { variants, .. } = &a.types[chain_id] else {
        panic!("recursive enum type info expected")
    };
    let ValueType::Tuple(next_tuple_id) = variants["Next"][0] else {
        panic!("tuple payload type expected")
    };
    let mut too_deep = Value::Enum {
        type_id: chain_id,
        tag: Rc::new("End".to_owned()),
        values: Rc::new(vec![]),
    };
    for _ in 0..16 {
        let tuple = tuple_value(&a, ValueType::Tuple(next_tuple_id), vec![too_deep]);
        too_deep = Value::Enum {
            type_id: chain_id,
            tag: Rc::new("Next".to_owned()),
            values: Rc::new(vec![tuple]),
        };
    }
    assert_eq!(
        run(&a, "host33", vec![too_deep]),
        Err(InvocationError::InvalidArguments)
    );
    fault(&a, "make33", "UBI-R0005");
    node(
        &a,
        r#"
const value=m.make32(); assert.equal(m.host32(JSON.stringify(value)),32);
let deep={tag:'End',values:[]}; for(let i=0;i<16;i++) deep={tag:'Next',values:[[deep]]};
assert.throws(()=>m.host33(JSON.stringify(deep)),TypeError);
assert.throws(()=>m.make33(),e=>e.code==='UBI-R0005');
"#,
    );
}

fn spellings(source: &str) -> Vec<String> {
    let lexed = lex("main.ubi", source).unwrap();
    let mut spans: Vec<_> = lexed
        .tokens
        .iter()
        .map(|token| &token.span)
        .chain(lexed.comments.iter().map(|comment| &comment.span))
        .collect();
    spans.sort_by_key(|span| span.start);
    spans
        .into_iter()
        .filter(|span| span.start != span.end)
        .map(|span| source[span.start..span.end].to_owned())
        .collect()
}

#[test]
fn tuple_formatter_preserves_comments_tokens_and_idempotence() {
    for source in [
        "// tuple header😀\nexport fn main()->(int,(string,)){/*before*/let (n,(text,)):(int,(string,))=(1,(\"ok\",)); // after\n(n,(text,))/*tail*/}\n// footer\n",
        "export fn main()->int{let (first,last)=(20,22);match((first==20,last==22)){(true,true)=>42,(true,false)=>0,(false,_)=>0}}",
    ] {
        let formatted = format_source(source).unwrap();
        assert_eq!(spellings(source), spellings(&formatted));
        assert_eq!(format_source(&formatted).unwrap(), formatted);
        assert!(formatted.ends_with('\n'));
    }
}

#[test]
fn malformed_tuple_types_use_parser_diagnostics() {
    for source in ["fn f(x: (int,, string)) {}", "fn f(x: (int, string) {}"] {
        let a = analysis(&[("main.ubi", source)]);
        assert!(
            a.diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code.starts_with("UBI000")),
            "{source}: {:#?}",
            a.diagnostics
        );
        assert!(crate::ir::lower(&a).is_err());
    }
}
