//! Independent conformance for SPEC section 25.
use crate::formatter::format_source;
use crate::generics_tests::{analysis, checked, node, rejected, run};
use crate::interpreter::{InvocationError, Value};
use crate::lexer::lex;

fn rejected_constraint(items: &[(&str, &str)], code: &str) {
    let a = analysis(items);
    assert!(
        a.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == code),
        "expected {code} for {items:?}: {:#?}",
        a.diagnostics
    );
    assert!(
        !a.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "UBI0090"),
        "constraint failure was mislabeled as resource exhaustion: {:#?}",
        a.diagnostics
    );
    assert!(crate::ir::lower(&a).is_err(), "invalid program lowered");
}

#[test]
fn constrained_generic_equality_has_fixed_interpreter_and_node_results() {
    let a = checked(&[(
        "main.ubi",
        r#"
fn includes<T: Equatable>(values: List<T>, needle: T) -> bool {
  values == [needle] && needle == needle
}

export fn main() -> bool { includes([7], 7) }
"#,
    )]);
    assert_eq!(run(&a, "main", vec![]), Ok(Value::Bool(true)));
    node(&a, "assert.equal(m.main(), true);");
}

#[test]
fn constrained_generic_exports_keep_the_host_boundary_and_concrete_wrappers_work() {
    let a = checked(&[(
        "main.ubi",
        r#"
export fn same<T: Equatable>(value: T) -> bool { value == value }
export fn concrete(value: int) -> bool { same(value) }
"#,
    )]);
    assert_eq!(
        run(&a, "same", vec![Value::Int(1)]),
        Err(InvocationError::InvalidArguments)
    );
    assert_eq!(
        run(&a, "concrete", vec![Value::Int(1)]),
        Ok(Value::Bool(true))
    );
    node(
        &a,
        "assert.throws(() => m.same(1), TypeError); assert.equal(m.concrete(1), true);",
    );
}

#[test]
fn bounded_parameter_formatting_preserves_tokens_comments_and_is_idempotent() {
    let source =
        "// bound declaration\nfn same<T/* parameter */:Equatable,>(value:T)->bool{value==value}\n";
    let spellings = |source: &str| {
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
            .collect::<Vec<_>>()
    };
    let formatted = format_source(source).unwrap();
    assert_eq!(spellings(source), spellings(&formatted));
    assert!(formatted.contains("// bound declaration"));
    assert!(formatted.contains("/* parameter */"));
    assert_eq!(format_source(&formatted).unwrap(), formatted);
}

#[test]
fn constrained_generics_support_membership_dedup_and_named_callbacks() {
    let a = checked(&[(
        "main.ubi",
        r#"
fn includes<T: Equatable,>(values: List<T>, needle: T) -> bool {
  let mut found = false;
  for (value in values) { if (value == needle) { found = true; } }
  found
}
fn unique<T: Equatable>(values: List<T>) -> List<T> {
  let mut result: List<T> = [];
  for (value in values) {
    if (!includes(result, value)) { result = append(result, value); }
  }
  result
}
fn reflexive<T: Equatable>(value: T) -> bool { value == value }
fn transitivelyEqual<T: Equatable>(value: T) -> bool { reflexive(value) }
fn apply(check: fn(int) -> bool, value: int) -> bool { check(value) }
fn captured<T: Equatable>(value: T) -> List<fn() -> bool> {
  let check = () => value == value;
  [check]
}
fn recur<T: Equatable>(value: T, n: int) -> bool {
  if (n == 0) { value == value } else { recur(value, n - 1) }
}
fn id<T>(value: T) -> T { value }
export fn main() -> bool {
  let values = unique([1, 2, 1, 3, 2]);
  let check: fn(int) -> bool = reflexive;
  let callbacks = captured((7, ()));
  let capturedResult = match (callbacks[0]) {
    Option.Some(callback) => callback(),
    Option.None => false,
  };
  includes(values, 3) && !includes(values, 4) && values == [1, 2, 3]
    && apply(reflexive, 9) && check(10) && transitivelyEqual((1, ()))
    && recur((2, ()), 5) && capturedResult && id(4) == 4
}
"#,
    )]);
    assert_eq!(run(&a, "main", vec![]), Ok(Value::Bool(true)));
    node(&a, "assert.equal(m.main(), true);");
}

#[test]
fn equality_bounds_cover_primitives_composites_and_recursive_nominals() {
    let a = checked(&[(
        "main.ubi",
        r#"
record Box<T> { value: T }
enum Choice<T> { Empty, Present(T) }
enum Chain<T> { End, Next(Chain<T>, T) }
fn equal<T: Equatable>(left: T, right: T) -> bool { left == right }
fn different<T: Equatable>(left: T, right: T) -> bool { left != right }
export fn composites() -> bool {
  let base: Map<string, (int,)> = mapEmpty();
  let mapA = mapSet(mapSet(base, "a", (1,)), "b", (2,));
  let mapB = mapSet(mapSet(base, "b", (2,)), "a", (1,));
  let boxed: Box<Option<(int,)>> = Box { value: Option.Some((3,)) };
  let choice: Choice<(int,)> = Choice.Present((4,));
  let chainA: Chain<int> = Chain.Next(Chain.End, 5);
  let chainB: Chain<int> = Chain.Next(Chain.End, 5);
  let nan = 0.0 / 0.0;
  let nanTuple = (nan, ());
  let nanList = [nan];
  let nanOption: Option<float> = Option.Some(nan);
  let floatMapBase: Map<string, float> = mapEmpty();
  let nanMap = mapSet(floatMapBase, "nan", nan);
  let nanBox: Box<float> = Box { value: nan };
  let nanChoice: Choice<float> = Choice.Present(nan);
  let nanChain: Chain<float> = Chain.Next(Chain.End, nan);
  equal(7, 7) && equal(2.5, 2.5) && equal(true, true)
    && equal("word", "word") && equal((), ())
    && equal([1, 2], [1, 2]) && equal(Option.Some((1,)), Option.Some((1,)))
    && equal((1, Option.Some("x")), (1, Option.Some("x")))
    && equal(mapA, mapB) && equal(boxed, Box { value: Option.Some((3,)) })
    && equal(choice, Choice.Present((4,))) && equal(chainA, chainB)
    && !equal(nan, nan) && different(nan, nan)
    && !equal(nanTuple, nanTuple) && !equal(nanList, nanList)
    && !equal(nanOption, nanOption) && !equal(nanMap, nanMap)
    && !equal(nanBox, nanBox) && !equal(nanChoice, nanChoice)
    && !equal(nanChain, nanChain)
}
"#,
    )]);
    assert_eq!(run(&a, "composites", vec![]), Ok(Value::Bool(true)));
    node(&a, "assert.equal(m.composites(), true);");
}

#[test]
fn constrained_records_enums_and_imports_keep_their_bounds() {
    let a = checked(&[
        (
            "library.ubi",
            r#"
export record Box<T: Equatable> { value: T }
export enum Choice<T: Equatable> { Empty, Present(T) }
export record Marker<T: Equatable> { number: int }
export enum Tag<T: Equatable> { Empty }
export fn equal<T: Equatable>(left: T, right: T) -> bool { left == right }
export fn identity<T: Equatable>(value: T) -> T { value }
export fn inspect<T: Equatable>(box: Box<T>, choice: Choice<T>) -> bool {
  box == box && choice == choice
}
"#,
        ),
        (
            "main.ubi",
            r#"
import { Box, Choice, Marker, Tag, equal, identity, inspect } from "./library.ubi";
export fn main() -> bool {
  let box = Box { value: (7, Option.Some("v")) };
  let choice: Choice<(int, Option<string>)> = Choice.Present((7, Option.Some("v")));
  let marker: Marker<int> = Marker { number: 8 };
  let tag: Tag<(int,)> = Tag.Empty;
  let importedCheck: fn(int, int) -> bool = equal;
  equal(box, Box { value: (7, Option.Some("v")) })
    && equal(choice, identity(choice)) && inspect(box, choice)
    && equal(marker, marker) && equal(tag, tag) && importedCheck(8, 8)
}
"#,
        ),
    ]);
    assert_eq!(run(&a, "main", vec![]), Ok(Value::Bool(true)));
    node(&a, "assert.equal(m.main(), true);");
}

#[test]
fn phantom_generic_equality_requires_equatable_nominal_arguments() {
    let a = checked(&[(
        "main.ubi",
        r#"
record Phantom<T> { marker: int }
fn equal<U: Equatable>(left: Phantom<U>, right: Phantom<U>) -> bool { left == right }
export fn main() -> bool {
  let value: Phantom<int> = Phantom { marker: 1 };
  equal(value, value)
}
"#,
    )]);
    assert_eq!(run(&a, "main", vec![]), Ok(Value::Bool(true)));
    node(&a, "assert.equal(m.main(), true);");

    rejected_constraint(
        &[(
            "main.ubi",
            "record Phantom<T> { marker: int } fn equal<U>(left: Phantom<U>, right: Phantom<U>) -> bool { left == right } export fn main() -> bool { let value: Phantom<fn() -> int> = Phantom { marker: 1 }; equal(value, value) }",
        )],
        "UBI0020",
    );
    rejected_constraint(
        &[(
            "main.ubi",
            "record Phantom<T> { marker: int } fn equal<U: Equatable>(left: Phantom<U>, right: Phantom<U>) -> bool { left == right } export fn main() -> bool { let value: Phantom<fn() -> int> = Phantom { marker: 1 }; equal(value, value) }",
        )],
        "UBI0020",
    );
}

#[test]
fn constraint_errors_are_precise_universal_and_not_resource_failures() {
    let unknown = "fn invalid<T: Missing>(value: T) -> bool { true }";
    let a = analysis(&[("main.ubi", unknown)]);
    let diagnostic = a
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "UBI0020")
        .expect("unknown bound should use UBI0020");
    assert_eq!(
        &unknown[diagnostic.primary.start..diagnostic.primary.end],
        "Missing"
    );
    assert!(crate::ir::lower(&a).is_err());

    for source in [
        "fn required<T: Equatable>(value: T) -> bool { value == value } fn middle<U>(value: U) -> bool { required(value) } fn outer<V>(value: V) -> bool { middle(value) } export fn main() -> bool { outer(1) }",
        "fn same<T>(left: T, right: T) -> bool { left == right } export fn main() -> bool { same(1, 1) }",
        "fn f<T: Equatable>(value: T) -> T { value + value }",
        "fn f<T: Equatable>(value: T) -> bool { value < value }",
        "fn f<T: Equatable>(value: T) -> Map<T, int> { mapEmpty() }",
        "record EqEmpty<T: Equatable> {} record Container { value: EqEmpty<Hidden> } record Hidden { callback: fn() -> int } export fn main() -> int { 0 }",
    ] {
        rejected_constraint(&[("main.ubi", source)], "UBI0020");
    }
    rejected(
        &[("main.ubi", "fn f<T:>(value: T) -> bool { true }")],
        "UBI0002",
    );
    rejected(
        &[("main.ubi", "fn f(value: Equatable) -> bool { true }")],
        "UBI0020",
    );
}

#[test]
fn functions_and_function_containing_aggregates_never_satisfy_equatable() {
    for source in [
        "fn required<T: Equatable>(value: T) -> bool { value == value } fn callback() -> int { 1 } export fn main() -> bool { required(callback) }",
        "fn required<T: Equatable>(value: T) -> bool { value == value } export fn main() -> bool { let callbacks: List<fn() -> int> = []; required(callbacks) }",
        "fn required<T: Equatable>(value: T) -> bool { value == value } export fn main() -> bool { let callback: Option<fn() -> int> = Option.None; required(callback) }",
        "enum E { Empty, Callback(fn() -> int) } fn required<T: Equatable>(value: T) -> bool { value == value } export fn main() -> bool { required(E.Empty) }",
        "record Marker<T: Equatable> { number: int } fn f() { let value: Marker<fn() -> int> = Marker { number: 1 }; }",
        "enum Tag<T: Equatable> { Empty } fn f() { let value: Tag<fn() -> int> = Tag.Empty; }",
        "fn required<T: Equatable>(value: T) -> bool { value == value } fn hide<T>() -> fn(T) -> bool { required } export fn main() -> bool { true }",
        "fn required<T: Equatable>(value: T) -> bool { value == value } fn hide<T>(value: T) -> List<fn(T) -> bool> { let callback: fn(T) -> bool = required; [callback] } export fn main() -> bool { true }",
        "fn required<T: Equatable>(value: T) -> bool { value == value } fn hide<T>(value: T) -> List<fn() -> bool> { let callback: fn(T) -> bool = required; let captured: fn() -> bool = () => callback(value); [captured] } export fn main() -> bool { true }",
    ] {
        rejected_constraint(&[("main.ubi", source)], "UBI0020");
    }
}

#[test]
fn constrained_type_parameters_do_not_admit_functions_ranges_or_capabilities() {
    for source in [
        "record Box<T: Equatable> { value: T } fn f(x: Box<fn() -> int>) {}",
        "enum Choice<T: Equatable> { Empty, Present(T) } fn f(x: Choice<fn() -> int>) {}",
        "fn required<T: Equatable>(value: T) -> bool { value == value } export fn main() -> bool { required(range(0, 1)) }",
    ] {
        rejected_constraint(&[("main.ubi", source)], "UBI0020");
    }
    rejected_constraint(
        &[(
            "main.ubi",
            "fn required<T: Equatable>(value: T) -> bool { value == value } fn use(c: Console) -> bool { required(c) }",
        )],
        "UBI0040",
    );
    rejected_constraint(
        &[(
            "main.ubi",
            "fn effectful(c: Console) -> int effects(console) { 1 } fn required<T: Equatable>(value: T) -> bool { value == value } fn use(c: Console) -> bool effects(console) { required(effectful) }",
        )],
        "UBI0040",
    );
}
