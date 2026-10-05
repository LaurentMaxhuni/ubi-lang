//! Independently authored conformance from SPEC section 18.
use crate::generics_tests::{analysis, checked, node, rejected, run};
use crate::interpreter::{InvocationError, Value};

#[test]
fn result_inference_matching_and_structural_equality_are_fixed() {
    let a = checked(&[(
        "main.ubi",
        r#"
fn ok<T,E>(x: T) -> Result<T,E> { Result.Ok(x) }
fn err<T,E>(x: E) -> Result<T,E> { Result.Err(x) }
fn wrapped<T,E>(x: Result<T,E>) -> Result<List<T>,E> { Result.Ok([x?]) }
fn empty() -> Result<List<int>,Option<string>> { ok([]) }
fn absent() -> Result<List<int>,Option<string>> { err(Option.None) }
fn score(x: Result<Option<bool>,string>) -> int {
  match (x) { Result.Ok(Option.None) => 1, Result.Ok(Option.Some(true)) => 2,
    Result.Ok(Option.Some(false)) => 3, Result.Err(s) => length(s) }
}
export fn results() -> bool {
  let a: Result<int,string> = ok(3); let b: Result<int,string> = err("no");
  a == Result.Ok(3) && b == Result.Err("no") && a != b
    && wrapped(a) == Result.Ok([3]) && wrapped(b) == Result.Err("no")
    && empty() == Result.Ok([]) && absent() == Result.Err(Option.None)
    && score(Result.Ok(Option.None)) == 1 && score(Result.Ok(Option.Some(true))) == 2
    && score(Result.Ok(Option.Some(false))) == 3 && score(Result.Err("😀a")) == 2
}
"#,
    )]);
    assert_eq!(run(&a, "results", vec![]), Ok(Value::Bool(true)));
    node(&a, "assert.equal(m.results(),true);");
    for source in [
        "fn f() { Result.Ok(1); }",
        "fn f() { Result.Err(\"e\"); }",
        "fn f(x: Result<int>) {}",
        "fn f(x: Result<int,string,bool>) {}",
        "fn f(x: Result<bool,string>) -> int { match (x) { Result.Ok(true) => 1, Result.Err(s) => 2 } }",
        "fn f(x: Result<int,string>) -> int { match (x) { Result.Ok(n) => n, Result.Err(s) if (true) => 0 } }",
        "fn f() -> Result<int,string> { Result.Err(false) }",
    ] { rejected(&[("main.ubi", source)], "UBI0020"); }
    rejected(&[("main.ubi", "enum Result<T,E> { A(T,E) }")], "UBI0011");
}

#[test]
fn propagation_exits_own_function_once_and_preserves_error_with_new_success_type() {
    let a = checked(&[(
        "main.ubi",
        r#"
record Pair { a: int, b: int }
fn input(b: bool) -> Result<int,string> { if (b) { Result.Ok(7) } else { Result.Err("stop😀") } }
fn flag() -> Result<bool,string> { Result.Err("guard") }
fn transform(b: bool) -> Result<string,string> { let n = input(b)?; Result.Ok(toString(n)) }
fn once() -> Result<int,int> {
  let mut n = 0; let x: Result<int,int> = Result.Ok(5);
  let v = { n = n + 1; x }?; Result.Ok(n * 10 + v)
}
fn throughLoop() -> Result<int,string> {
  for (n in [1,2,3]) { let x = input(false)?; 1 / 0; }
  Result.Ok(1 / 0)
}
fn throughArguments() -> Result<Pair,string> {
  Result.Ok(Pair { a: input(false)?, b: 1 / 0 })
}
fn throughGuard() -> Result<int,string> {
  let n = match (1) { _ if (flag()?) => 1 / 0, _ => 1 / 0 }; Result.Ok(n)
}
fn throughCall() -> Result<int,string> { Result.Ok(add(input(false)?, 1 / 0)) }
fn add(a: int,b: int) -> int { a + b }
export fn success() -> bool { transform(true) == Result.Ok("7") && once() == Result.Ok(15) }
export fn ownBoundary() -> int {
  let mut n = 2;
  let v = match (transform(false)) { Result.Ok(x) => 0, Result.Err(e) => length(e) };
  n = n + 10; n + v
}
export fn shortCircuit() -> bool {
  transform(false) == Result.Err("stop😀") && throughLoop() == Result.Err("stop😀")
    && throughArguments() == Result.Err("stop😀") && throughGuard() == Result.Err("guard")
    && throughCall() == Result.Err("stop😀")
}
export fn fatal() -> Result<int,string> { let x: Result<int,string> = Result.Ok(1 / 0); Result.Ok(x?) }
"#,
    )]);
    assert_eq!(run(&a, "success", vec![]), Ok(Value::Bool(true)));
    assert_eq!(run(&a, "ownBoundary", vec![]), Ok(Value::Int(17)));
    assert_eq!(run(&a, "shortCircuit", vec![]), Ok(Value::Bool(true)));
    let Err(InvocationError::Runtime(fault)) = run(&a, "fatal", vec![]) else {
        panic!("fatal fault was intercepted");
    };
    assert_eq!(fault.code, "UBI-R0002");
    node(&a, "assert.equal(m.success(),true); assert.equal(m.ownBoundary(),17); assert.equal(m.shortCircuit(),true); assert.throws(()=>m.fatal(),e=>e.code==='UBI-R0002');");
}

#[test]
fn invalid_propagation_contexts_report_question_token() {
    for source in [
        "fn f() -> int { let r: Result<int,string> = Result.Ok(1); r? }",
        "fn f() { let r: Result<int,string> = Result.Ok(1); r?; }",
        "fn f() -> Result<int,string> { let r: Result<int,int> = Result.Err(3); Result.Ok(r?) }",
        "fn f() -> Result<int,string> { let r: Option<int> = Option.Some(1); Result.Ok(r?) }",
        "fn f() -> Result<int,string> { Result.Ok(3?) }",
        "fn f() -> Result<int,string> { let r: Result<int,string> = Result.Ok(1); let x = () => r?; Result.Ok(1) }",
        "fn f<T,E,F>(r: Result<T,E>) -> Result<T,F> { Result.Ok(r?) }",
    ] {
        let a = analysis(&[("main.ubi", source)]);
        let diagnostic = a.diagnostics.iter().find(|d| d.code == "UBI0033")
            .unwrap_or_else(|| panic!("{source}: {:#?}", a.diagnostics));
        assert_eq!(&source[diagnostic.primary.start..diagnostic.primary.end], "?");
        assert!(crate::ir::lower(&a).is_err());
    }
}

#[test]
fn result_host_envelopes_are_exact_and_payloads_frozen() {
    let a = checked(&[(
        "main.ubi",
        r#"
export record Error { message: string }
export fn echo(x: Result<List<Option<int>>,Error>) -> Result<List<Option<int>>,Error> { x }
export fn make() -> Result<List<Option<int>>,Error> { Result.Ok([Option.Some(4),Option.None]) }
export fn fail() -> Result<List<Option<int>>,Error> { Result.Err(Error { message: "bad😀" }) }
"#,
    )]);
    for name in ["make", "fail"] {
        let v = run(&a, name, vec![]).unwrap();
        assert_eq!(run(&a, "echo", vec![v.clone()]), Ok(v));
    }
    assert_eq!(
        run(&a, "echo", vec![Value::Int(1)]),
        Err(InvocationError::InvalidArguments)
    );
    node(
        &a,
        r#"
const out=m.echo('{"tag":"Ok","values":[[{"tag":"Some","value":4},{"tag":"None"}]]}');
assert.deepEqual(out,m.make());
for(const x of [out,out.values,out.values[0],out.values[0][0]]) assert.ok(Object.isFrozen(x));
const error=m.echo('{"tag":"Err","values":[{"message":"bad😀"}]}');
assert.deepEqual(error,m.fail()); assert.ok(Object.isFrozen(error.values[0]));
for(const bad of [out,'{"tag":"Ok","value":1}','{"tag":"Ok","values":[]}','{"tag":"Err","values":[{"message":1}]}','{"tag":"Missing","values":[0]}','{"tag":"Ok","values":[[],0]}','{"tag":"Ok","values":[[]],"extra":0}']) assert.throws(()=>m.echo(bad),TypeError);
"#,
    );
}
