//! Public independent SPEC section 21 conformance. Expectations precede implementation review.
use crate::analyzer::CapabilityKind;
use crate::generics_tests::{checked, node, rejected, run};
use crate::interpreter::{HostCapability, InvocationError, Value};
use std::cell::RefCell;
use std::rc::Rc;

#[test]
fn async_requires_annotations_immediate_direct_await_and_named_context() {
    rejected(&[("main.ubi", "async fn f() { 1 }")], "UBI0021");
    for source in [
        "async fn value() -> int { 1 } fn f() -> int { await value() }",
        "async fn value() -> int { 1 } async fn f() -> int { value() }",
        "fn value() -> int { 1 } async fn f() -> int { await value() }",
        "async fn f() -> int { await 1 }",
        "async fn f() -> int { let arrow = async () => 1; 1 }",
        "async fn value() -> int { 1 } async fn f() -> int { let alias = value; await alias() }",
        "async fn value() -> int { 1 } fn f() { map([1], value); }",
        "async fn value() -> int { 1 } async fn f() -> int { let arrow = () => await value(); arrow() }",
        "async fn value() -> int { 1 } async fn f() -> int { let arrow = () => value(); arrow() }",
        "async fn value() -> int { 1 } async fn f(b: bool) -> int { await (if (b) { value } else { value })() }",
        "async fn value() -> int { 1 } fn id<T>(x: T) -> T { x } async fn f() -> int { let alias = id(value); await alias() }",
        "async fn f(s: Storage) -> Result<Option<string>,HostError> effects(storage) { storageGetAsync(s,\"x\") }",
        "fn f(s: Storage) -> Result<Option<string>,HostError> effects(storage) { await storageGetAsync(s,\"x\") }",
    ] { rejected(&[("main.ubi", source)], "UBI0041"); }
    rejected(&[
        ("lib.ubi", "export async fn value() -> int { 1 }"),
        ("main.ubi", "import { value } from \"./lib.ubi\"; async fn f() -> int { let alias = value; await alias() }"),
    ], "UBI0041");
}

#[test]
fn awaited_effects_propagate_through_dead_branches_imports_and_recursion() {
    for source in [
        "async fn f(h: Http) -> Result<string,HostError> { await httpGet(h,\"url\") }",
        "async fn f(s: Storage) -> Result<Option<string>,HostError> { await storageGetAsync(s,\"key\") }",
        "async fn leaf(h: Http) -> Result<string,HostError> effects(http) { await httpGet(h,\"url\") } async fn f(h: Http) -> Result<string,HostError> { if (false) { await leaf(h) } else { Result.Ok(\"ok\") } }",
        "async fn f(h: Http,n: int) -> Result<string,HostError> { if (n == 0) { Result.Ok(\"ok\") } else { await g(h,n - 1) } } async fn g(h: Http,n: int) -> Result<string,HostError> effects(http) { await httpGet(h,\"url\"); await f(h,n) }",
        "async fn f(s: Storage) -> Result<string,HostError> effects(http) { await httpGet(s,\"url\") }",
        "record Box { h: Http }",
        "fn f(h: Http) -> Http { h }",
        "fn id<T>(x: T) -> T { x } fn f(h: Http) { id(h); }",
    ] { rejected(&[("main.ubi", source)], "UBI0040"); }
    rejected(&[
        ("lib.ubi", "export async fn leaf(h: Http) -> Result<string,HostError> effects(http) { await httpGet(h,\"url\") }"),
        ("main.ubi", "import { leaf } from \"./lib.ubi\"; export async fn f(h: Http) -> Result<string,HostError> { await leaf(h) }"),
    ], "UBI0040");
}

const SOURCE: &str = r#"
fn plus(a: int,b: int) -> int { a + b }
async fn number(n: int) -> int { n }
export async fn pure() -> int {
  let mut n = 0;
  let value = await number(plus({ n = n + 1; n }, { n = n * 10 + 2; n }));
  value * 100 + n
}
export async fn sequence(s: Storage,h: Http) -> Result<int,HostError> effects(storage,http) {
  (await storageSetAsync(s,"key","value"))?;
  let found = (await storageGetAsync(s,"key"))?;
  let text = (await httpGet(h,"url"))?;
  (await storageRemoveAsync(s,"key"))?;
  Result.Ok(length(text) + match (found) { Option.Some(value) => length(value), Option.None => 0 })
}
export async fn status(h: Http) -> int effects(http) {
  match (await httpGet(h,"url")) {
    Result.Ok(text) => length(text), Result.Err(HostError.Failure(_)) => -1,
    Result.Err(HostError.Decode(_)) => -2, Result.Err(HostError.Cancelled(_)) => -3
  }
}
export async fn first(s: Storage) -> Result<int,HostError> effects(storage) {
  (await storageSetAsync(s,"first","value"))?;
  (await storageSetAsync(s,"never","value"))?;
  Result.Ok(1 / 0)
}
export async fn loopStop(s: Storage) -> Result<int,HostError> effects(storage) {
  for (n in [1,2]) { (await storageSetAsync(s,toString(n),"value"))?; }
  Result.Ok(1 / 0)
}
export async fn argumentStop(s: Storage) -> Result<int,HostError> effects(storage) {
  Result.Ok(plus({ (await storageSetAsync(s,"first","value"))?; 1 },1 / 0))
}
export async fn argumentFault(h: Http) -> Result<string,HostError> effects(http) { await httpGet(h,toString(1 / 0)) }
export async fn recursive(n: int) -> int { if (n == 0) { 0 } else { await recursive(n - 1) + 1 } }
export async fn recursiveMatch(n: int) -> int { match (n) { 0 => 0, _ => await recursiveMatch(n - 1) + 1 } }
export async fn recursiveResult(n: int) -> Result<int,string> { if (n == 0) { Result.Ok(0) } else { Result.Ok((await recursiveResult(n - 1))? + 1) } }
async fn flag() -> bool { true }
export async fn nested() -> int {
  let x = match (1) { _ if (await flag()) => await number(7), _ => 0 };
  plus(await number(if (await flag()) { 3 } else { 4 }),x)
}
export async fn exhausted() -> int { let mut n = 0; while (n < 2000000) { n = await number(n + 1); } n }
"#;

#[test]
fn interpreter_completed_fixtures_preserve_order_once_propagation_and_faults() {
    let a = checked(&[("main.ubi", SOURCE)]);
    assert_eq!(run(&a, "pure", vec![]), Ok(Value::Int(1312)));
    let calls = Rc::new(RefCell::new(Vec::new()));
    let log = calls.clone();
    let storage = Value::Capability(Rc::new(HostCapability::new(
        CapabilityKind::Storage,
        move |op, args| {
            log.borrow_mut().push((op.to_owned(), args.to_vec()));
            Ok(if op == "storageGetAsync" {
                r#"{"tag":"Ok","values":[{"tag":"Some","value":"value"}]}"#
            } else {
                r#"{"tag":"Ok","values":[null]}"#
            }
            .into())
        },
    )));
    let log = calls.clone();
    let http = Value::Capability(Rc::new(HostCapability::new(
        CapabilityKind::Http,
        move |op, args| {
            log.borrow_mut().push((op.to_owned(), args.to_vec()));
            Ok(r#"{"tag":"Ok","values":["abc"]}"#.into())
        },
    )));
    let result = run(&a, "sequence", vec![storage, http.clone()]).unwrap();
    let Value::Enum { tag, values, .. } = result else {
        panic!("Result expected");
    };
    assert_eq!(&*tag, "Ok");
    assert_eq!(values.as_slice(), &[Value::Int(8)]);
    assert_eq!(
        calls
            .borrow()
            .iter()
            .map(|(op, _)| op.as_str())
            .collect::<Vec<_>>(),
        [
            "storageSetAsync",
            "storageGetAsync",
            "httpGet",
            "storageRemoveAsync"
        ]
    );
    calls.borrow_mut().clear();
    let Err(InvocationError::Runtime(fault)) = run(&a, "argumentFault", vec![http]) else {
        panic!("fault expected");
    };
    assert_eq!(fault.code, "UBI-R0002");
    assert!(calls.borrow().is_empty());
    let log = calls.clone();
    let failing = Value::Capability(Rc::new(HostCapability::new(
        CapabilityKind::Storage,
        move |_, args| {
            log.borrow_mut().push(("failure".into(), args.to_vec()));
            Ok(r#"{"tag":"Err","values":[{"tag":"Failure","values":["disk"]}]}"#.into())
        },
    )));
    let Value::Enum { tag, .. } = run(&a, "first", vec![failing]).unwrap() else {
        panic!("Result expected");
    };
    assert_eq!(&*tag, "Err");
    assert_eq!(calls.borrow().len(), 1);
    assert_eq!(run(&a, "recursive", vec![Value::Int(4)]), Ok(Value::Int(4)));
    for (name, args) in [("recursive", vec![Value::Int(40)]), ("exhausted", vec![])] {
        let Err(InvocationError::Runtime(fault)) = run(&a, name, args) else {
            panic!("budget expected");
        };
        assert_eq!(fault.code, "UBI-R0005");
    }
}

#[test]
fn javascript_delayed_hosts_are_sequential_and_snapshot_callbacks() {
    let a = checked(&[("main.ubi", SOURCE)]);
    node(
        &a,
        r#"
const host=await import('./.ubi-host.mjs');
const ok=x=>JSON.stringify({tag:'Ok',values:[x]});
const calls=[]; let resolveSet;
const callbacks={set:(key,value)=>{calls.push(['set',key,value]);return new Promise(r=>resolveSet=r);},get:async key=>{calls.push(['get',key]);return ok({tag:'Some',value:'value'});},remove:async key=>{calls.push(['remove',key]);return ok(null);}};
const s=host.createAsyncStorageCapability(callbacks);
callbacks.get=()=>{throw Error('mutated');};
const h=host.createHttpCapability(async url=>{calls.push(['http',url]);return ok('abc');});
const pending=m.sequence(s,h); assert.ok(pending instanceof Promise);
assert.deepEqual(calls,[['set','key','value']]);
await Promise.resolve(); assert.equal(calls.length,1);
resolveSet(ok(null)); assert.deepEqual(await pending,{tag:'Ok',values:[8]});
assert.deepEqual(calls,[['set','key','value'],['get','key'],['http','url'],['remove','key']]);
assert.equal(await m.pure(),1312); assert.equal(await m.recursive(4),4);
let writes=0; const fail=host.createAsyncStorageCapability({get:async()=>ok({tag:'None'}),remove:async()=>ok(null),set:async()=>{writes++;return JSON.stringify({tag:'Err',values:[{tag:'Failure',values:['disk']}]});}});
assert.deepEqual(await m.first(fail),{tag:'Err',values:[{tag:'Failure',values:['disk']}]});assert.equal(writes,1);
for(const name of ['loopStop','argumentStop']) { writes=0;assert.deepEqual(await m[name](fail),{tag:'Err',values:[{tag:'Failure',values:['disk']}]});assert.equal(writes,1); }
"#,
    );
}

#[test]
fn javascript_cancellation_failure_decode_faults_and_validation_are_distinct() {
    let a = checked(&[("main.ubi", SOURCE)]);
    node(
        &a,
        r#"
const host=await import('./.ubi-host.mjs'); const ok=x=>JSON.stringify({tag:'Ok',values:[x]});
const cap=get=>host.createHttpCapability(get);
assert.equal(await m.status(cap(async()=>ok('😀a'))),2);
assert.equal(await m.status(cap(async()=>JSON.stringify({tag:'Err',values:[{tag:'Failure',values:['network']}]}))),-1);
for(const bad of ['{','null','{"tag":"Ok","values":[true]}','{"tag":"Ok","values":["ok"],"extra":0}',{},3]) assert.equal(await m.status(cap(async()=>bad)),-2);
for(const get of [async()=>{throw Error('reject');},()=>{throw Error('throw');}]) await assert.rejects(m.status(cap(get)),e=>e.code==='UBI-R0004');
let calls=0;const controller=new AbortController();controller.abort();
assert.equal(await m.status(host.createHttpCapability(async()=>{calls++;return ok('late');},{signal:controller.signal})),-3);assert.equal(calls,0);
const active=new AbortController();let resolve;let received;let listeners=0;
const add=active.signal.addEventListener.bind(active.signal),remove=active.signal.removeEventListener.bind(active.signal);
active.signal.addEventListener=(...args)=>{if(args[0]==='abort')listeners++;return add(...args);};
active.signal.removeEventListener=(...args)=>{if(args[0]==='abort')listeners--;return remove(...args);};
const pending=m.status(host.createHttpCapability((url,signal)=>{received=signal;return new Promise(r=>resolve=r);},{signal:active.signal}));
assert.equal(received,active.signal);active.abort();assert.equal(await pending,-3);assert.equal(listeners,0);resolve(ok('late'));await Promise.resolve();
let writes=0;const cancelled=new AbortController();let resume;
const s=host.createAsyncStorageCapability({get:async()=>ok({tag:'None'}),remove:async()=>ok(null),set:()=>{writes++;return new Promise(r=>resume=r);}},{signal:cancelled.signal});
const first=m.first(s);cancelled.abort();assert.equal((await first).values[0].tag,'Cancelled');resume(ok(null));await new Promise(r=>setImmediate(r));assert.equal(writes,1);
const h=cap(async()=>{calls++;return ok('ok');});calls=0;
for(const fake of [{},'{}',Object.freeze({})]) await assert.rejects(async()=>m.status(fake),TypeError);
await assert.rejects(m.argumentFault(h),e=>e.code==='UBI-R0002');assert.equal(calls,0);
"#,
    );
}

#[test]
fn imported_generic_async_functions_and_branch_awaits_preserve_values() {
    let a = checked(&[
        ("lib.ubi", "export async fn id<T>(value: T) -> T { value }"),
        (
            "main.ubi",
            r#"import { id } from "./lib.ubi"; export async fn main(b: bool) -> int { let mut n = 0; let x = if (b) { await id({ n = n + 1; 7 }) } else { await id({ n = n + 2; 3 }) }; x * 10 + n }"#,
        ),
    ]);
    assert_eq!(run(&a, "main", vec![Value::Bool(true)]), Ok(Value::Int(71)));
    assert_eq!(
        run(&a, "main", vec![Value::Bool(false)]),
        Ok(Value::Int(32))
    );
    node(
        &a,
        "assert.equal(await m.main(true),71);assert.equal(await m.main(false),32);",
    );
}

#[test]
fn javascript_budgets_survive_await_and_concurrent_exports_have_independent_frames() {
    let a = checked(&[("main.ubi", SOURCE)]);
    node(
        &a,
        r#"
assert.deepEqual(await Promise.all(Array.from({length:8},()=>m.recursive(20))),Array(8).fill(20));
await assert.rejects(m.recursive(40),e=>e.code==='UBI-R0005');
await assert.rejects(m.exhausted(),e=>e.code==='UBI-R0005');
assert.equal(await m.recursive(4),4);
assert.equal(await m.nested(),10);
for(const name of ['recursiveMatch','recursiveResult']) await assert.rejects(m[name](40),e=>e.code==='UBI-R0005');
"#,
    );
}

#[test]
fn interpreter_nested_await_and_recursive_match_result_keep_frame_budget() {
    let a = checked(&[("main.ubi", SOURCE)]);
    assert_eq!(run(&a, "nested", vec![]), Ok(Value::Int(10)));
    for name in ["recursiveMatch", "recursiveResult"] {
        let Err(InvocationError::Runtime(fault)) = run(&a, name, vec![Value::Int(40)]) else {
            panic!("frame budget expected");
        };
        assert_eq!(fault.code, "UBI-R0005");
    }
}
