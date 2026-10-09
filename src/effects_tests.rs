//! Independently authored SPEC section 20 escape attempts and permitted controls.
use crate::analyzer::CapabilityKind;
use crate::generics_tests::{checked, node, rejected, run};
use crate::interpreter::{HostCapability, InvocationError, Value};
use std::cell::RefCell;
use std::rc::Rc;

#[test]
fn effect_escape_suite_rejects_omissions_callbacks_aliases_and_smuggled_capabilities() {
    for source in [
        "fn f(c: Console) -> Result<unit,HostError> { consoleLog(c,\"x\") }",
        "fn leaf(c: Console) -> Result<unit,HostError> effects(console) { consoleLog(c,\"x\") } fn f(c: Console) -> Result<unit,HostError> { leaf(c) }",
        "fn f(c: Console) -> Result<unit,HostError> { if (false) { consoleLog(c,\"x\") } else { Result.Ok(()) } }",
        "fn f(c: Console,n: int) -> Result<unit,HostError> { if (n == 0) { Result.Ok(()) } else { g(c,n - 1) } } fn g(c: Console,n: int) -> Result<unit,HostError> effects(console) { consoleLog(c,\"x\")?; f(c,n) }",
        "fn leaf(c: Console) -> Result<unit,HostError> effects(console) { consoleLog(c,\"x\") } fn f(c: Console) -> Result<unit,HostError> effects(console) { let hidden = leaf; hidden(c) }",
        "fn leaf(c: Console) -> Result<unit,HostError> effects(console) { consoleLog(c,\"x\") } fn f(c: Console) effects(console) { let closure = () => leaf(c); closure(); }",
        "fn leaf(c: Console,n: int) -> int effects(console) { consoleLog(c,\"x\"); n } fn f(c: Console) effects(console) { map([1],(n: int) => leaf(c,n)); }",
        "fn f(c: Console) { let hidden = () => c; }",
        "record Box<T> { value: T } fn f(c: Console) { Box { value: c }; }",
        "record Box { value: Console }",
        "enum E { A(Storage) }",
        "fn f(c: Console) { [c]; }",
        "fn f(c: Console) { let x: Option<Console> = Option.Some(c); }",
        "fn f(c: Console) { let x: Map<string,Console> = mapEmpty(); }",
        "fn f(c: Console) -> Console { c }",
        "fn f(c: Console) -> bool { c == c }",
        "fn id<T>(x: T) -> T { x } fn f(c: Console) { id(c); }",
        "fn plain(c: Console) -> int { 1 } fn generic<T>(c: T) -> int { let capture = () => c; 2 } export fn smuggle(c: Console) -> int { (if (false) { plain } else { generic })(c) }",
        "fn plain(c: Console) -> int { 1 } fn generic<T>(c: T) -> int { let capture = () => c; 2 } export fn smuggle(c: Console) -> int { let invoke = if (false) { plain } else { generic }; invoke(c) }",
        "fn plain(s: Storage) -> int { 1 } fn generic<T>(s: T) -> int { let capture = () => s; 2 } export fn smuggle(s: Storage) -> int { (if (false) { plain } else { generic })(s) }",
        "fn f(s: Storage) -> Result<unit,HostError> effects(console) { consoleLog(s,\"x\") }",
        "fn f() -> Result<unit,HostError> effects(console) { consoleLog((),\"x\") }",
        "fn f() -> Result<unit,HostError> effects(console) { consoleLog(\"x\") }",
        "fn f(c: Console) effects(network) {}",
    ] { rejected(&[("main.ubi", source)], "UBI0040"); }
    rejected(
        &[("main.ubi", "fn f(c: Console) effects(console,console) {}")],
        "UBI0011",
    );
    rejected(&[
        ("host.ubi", "export fn write(c: Console) -> Result<unit,HostError> effects(console) { consoleLog(c,\"x\") }"),
        ("main.ubi", "import { write } from \"./host.ubi\"; export fn f(c: Console) -> Result<unit,HostError> { write(c) }"),
    ], "UBI0040");
    for source in [
        "fn f() { console.log(\"x\"); }",
        "fn f() { localStorage.getItem(\"x\"); }",
        "fn f() { fetch(\"x\"); }",
    ] {
        rejected(&[("main.ubi", source)], "UBI0010");
    }
    for source in [
        "async fn f() {}",
        "fn f() { await 1; }",
        "export { f }; fn f() {}",
    ] {
        let a = crate::generics_tests::analysis(&[("main.ubi", source)]);
        assert!(
            !a.diagnostics.is_empty(),
            "unsupported escape accepted: {source}"
        );
        assert!(crate::ir::lower(&a).is_err());
    }
}

#[test]
fn composite_pure_generic_callees_remain_permitted() {
    let a = checked(&[(
        "main.ubi",
        r#"
fn plain(x: int) -> int { x + 1 }
fn generic<T>(x: T) -> int { let capture = () => x; 2 }
export fn composite(b: bool) -> int { (if (b) { plain } else { generic })(5) }
export fn alias(b: bool) -> int { let invoke = if (b) { plain } else { generic }; invoke(5) }
"#,
    )]);
    for name in ["composite", "alias"] {
        assert_eq!(run(&a, name, vec![Value::Bool(true)]), Ok(Value::Int(6)));
        assert_eq!(run(&a, name, vec![Value::Bool(false)]), Ok(Value::Int(2)));
    }
    node(&a, "assert.equal(m.composite(true),6); assert.equal(m.composite(false),2); assert.equal(m.alias(true),6); assert.equal(m.alias(false),2);");
}

const FIXTURE: &str = r#"
fn leaf(c: Console,s: Storage) -> Result<int,HostError> effects(console,storage) {
  consoleLog(c,"start")?; storageSet(s,"task","hello😀")?;
  let found = storageGet(s,"task")?; storageRemove(s,"task")?;
  Result.Ok(match (found) { Option.Some(text) => length(text), Option.None => 0 })
}
export fn run(c: Console,s: Storage) -> int effects(console,storage) {
  let console = c; let storage = s;
  match (leaf(console,storage)) { Result.Ok(n) => n, Result.Err(_) => -1 }
}
export fn get(s: Storage) -> Result<Option<string>,HostError> effects(storage) { storageGet(s,"task") }
export fn status(s: Storage) -> int effects(storage) {
  match (storageGet(s,"task")) {
    Result.Ok(Option.Some(text)) => length(text), Result.Ok(Option.None) => 0,
    Result.Err(HostError.Failure(message)) => length(message), Result.Err(HostError.Decode(_)) => -2,
    Result.Err(HostError.Cancelled(_)) => -3
  }
}
export fn validate(c: Console,n: int) -> Result<unit,HostError> effects(console) { consoleLog(c,toString(n)) }
export fn faultBefore(c: Console) -> Result<unit,HostError> effects(console) { consoleLog(c,toString(1 / 0)) }
export fn recursive(c: Console,n: int) -> Result<int,HostError> effects(console) {
  if (n == 0) { Result.Ok(0) } else { consoleLog(c,toString(n))?; Result.Ok(recursive(c,n - 1)? + 1) }
}
export fn shadow() -> int {
  let consoleLog = (n: int) => n + 1; consoleLog(4)
}
"#;

#[test]
fn permitted_capabilities_run_in_order_with_aliases_recursion_and_shadowing() {
    let a = checked(&[("main.ubi", FIXTURE)]);
    let calls = Rc::new(RefCell::new(Vec::<(String, Vec<String>)>::new()));
    let state = Rc::new(RefCell::new(None::<String>));
    let log = calls.clone();
    let console = Value::Capability(Rc::new(HostCapability::new(
        CapabilityKind::Console,
        move |op, args| {
            log.borrow_mut().push((op.into(), args.to_vec()));
            Ok("{\"tag\":\"Ok\",\"values\":[null]}".into())
        },
    )));
    let log = calls.clone();
    let stored = state.clone();
    let storage = Value::Capability(Rc::new(HostCapability::new(
        CapabilityKind::Storage,
        move |op, args| {
            log.borrow_mut().push((op.into(), args.to_vec()));
            match op {
                "storageSet" => {
                    *stored.borrow_mut() = Some(args[1].clone());
                    Ok("{\"tag\":\"Ok\",\"values\":[null]}".into())
                }
                "storageGet" => Ok(match &*stored.borrow() {
                    Some(text) => format!(
                        "{{\"tag\":\"Ok\",\"values\":[{{\"tag\":\"Some\",\"value\":{}}}]}}",
                        serde_json::to_string(text).unwrap()
                    ),
                    None => "{\"tag\":\"Ok\",\"values\":[{\"tag\":\"None\"}]}".into(),
                }),
                "storageRemove" => {
                    *stored.borrow_mut() = None;
                    Ok("{\"tag\":\"Ok\",\"values\":[null]}".into())
                }
                _ => panic!("unexpected operation: {op}"),
            }
        },
    )));
    assert_eq!(
        run(&a, "run", vec![console.clone(), storage.clone()]),
        Ok(Value::Int(6))
    );
    assert_eq!(
        &*calls.borrow(),
        &[
            ("consoleLog".into(), vec!["start".into()]),
            ("storageSet".into(), vec!["task".into(), "hello😀".into()]),
            ("storageGet".into(), vec!["task".into()]),
            ("storageRemove".into(), vec!["task".into()]),
        ]
    );
    assert_eq!(*state.borrow(), None);
    calls.borrow_mut().clear();
    assert_eq!(
        run(&a, "validate", vec![console.clone(), Value::Bool(true)]),
        Err(InvocationError::InvalidArguments)
    );
    assert_eq!(
        run(&a, "validate", vec![storage, Value::Int(1)]),
        Err(InvocationError::InvalidArguments)
    );
    assert!(calls.borrow().is_empty());
    let Err(InvocationError::Runtime(fault)) = run(&a, "faultBefore", vec![console.clone()]) else {
        panic!("expected source fault");
    };
    assert_eq!(fault.code, "UBI-R0002");
    assert!(calls.borrow().is_empty());
    let recursive = run(&a, "recursive", vec![console, Value::Int(3)]).unwrap();
    let Value::Enum { tag, values, .. } = recursive else {
        panic!("Result expected");
    };
    assert_eq!(&*tag, "Ok");
    assert_eq!(values.as_slice(), &[Value::Int(3)]);
    assert_eq!(
        calls
            .borrow()
            .iter()
            .map(|(_, args)| args[0].clone())
            .collect::<Vec<_>>(),
        ["3", "2", "1"]
    );
    assert_eq!(run(&a, "shadow", vec![]), Ok(Value::Int(5)));
    node(
        &a,
        r#"
const host=await import('./.ubi-host.mjs');
const calls=[]; let value;
const ok=x=>JSON.stringify({tag:'Ok',values:[x]});
const c=host.createConsoleCapability(text=>{calls.push(['consoleLog',text]); return ok(null);});
const callbacks={
  get:key=>{calls.push(['storageGet',key]); return ok(value===undefined?{tag:'None'}:{tag:'Some',value});},
  set:(key,text)=>{calls.push(['storageSet',key,text]); value=text; return ok(null);},
  remove:key=>{calls.push(['storageRemove',key]); value=undefined; return ok(null);}
};
const s=host.createStorageCapability(callbacks); callbacks.get=()=>{throw Error('mutated');};
assert.ok(Object.isFrozen(c)); assert.ok(Object.isFrozen(s));
assert.equal(m.run(c,s),6); assert.equal(value,undefined);
assert.deepEqual(calls,[['consoleLog','start'],['storageSet','task','hello😀'],['storageGet','task'],['storageRemove','task']]);
const decoded=m.get(s); assert.deepEqual(decoded,{tag:'Ok',values:[{tag:'None'}]});
for(const x of [decoded,decoded.values,decoded.values[0]]) assert.ok(Object.isFrozen(x));
calls.length=0;
for(const args of [[c,true],[s,1],[{},1],['{}',1],[Object.freeze({}),1]]) assert.throws(()=>m.validate(...args),TypeError);
let reads=0; const fake=new Proxy({}, {get(){reads++;throw Error('get');},ownKeys(){reads++;throw Error('keys');}});
assert.throws(()=>m.validate(fake,1),TypeError); assert.equal(reads,0);
assert.equal(calls.length,0); assert.throws(()=>m.faultBefore(c),e=>e.code==='UBI-R0002'); assert.equal(calls.length,0);
assert.deepEqual(m.recursive(c,3),{tag:'Ok',values:[3]}); assert.deepEqual(calls,[['consoleLog','3'],['consoleLog','2'],['consoleLog','1']]);
assert.equal(m.shadow(),5);
"#,
    );
}

#[test]
fn adapter_failures_decode_errors_and_unexpected_throws_keep_distinct_outcomes() {
    let a = checked(&[("main.ubi", FIXTURE)]);
    let capability = |text: &'static str| {
        Value::Capability(Rc::new(HostCapability::new(
            CapabilityKind::Storage,
            move |_, _| Ok(text.into()),
        )))
    };
    assert_eq!(
        run(
            &a,
            "status",
            vec![capability(
                "{\"tag\":\"Err\",\"values\":[{\"tag\":\"Failure\",\"values\":[\"disk😀\"]}]}"
            )]
        ),
        Ok(Value::Int(5))
    );
    for malformed in [
        "{",
        "null",
        "{\"tag\":\"Ok\",\"values\":[true]}",
        "{\"tag\":\"Ok\",\"values\":[{\"tag\":\"Some\",\"value\":3}]}",
        "{\"tag\":\"Err\",\"values\":[{\"tag\":\"Failure\",\"values\":[true]}]}",
    ] {
        assert_eq!(
            run(&a, "status", vec![capability(malformed)]),
            Ok(Value::Int(-2))
        );
    }
    let thrown = Value::Capability(Rc::new(HostCapability::new(
        CapabilityKind::Storage,
        |_, _| Err(()),
    )));
    let Err(InvocationError::Runtime(fault)) = run(&a, "status", vec![thrown]) else {
        panic!("unexpected adapter throw recovered");
    };
    assert_eq!(fault.code, "UBI-R0004");
    node(
        &a,
        r#"
const host=await import('./.ubi-host.mjs');
const cap=get=>host.createStorageCapability({get,set:()=>'',remove:()=>''});
const failure=JSON.stringify({tag:'Err',values:[{tag:'Failure',values:['disk😀']}]});
assert.equal(m.status(cap(()=>failure)),5);
for(const bad of ['{','null','{"tag":"Ok","values":[true]}','{"tag":"Ok","values":[{"tag":"Some","value":3}]}','{"tag":"Err","values":[{"tag":"Failure","values":[true]}]}',{},Promise.resolve('ok')]) assert.equal(m.status(cap(()=>bad)),-2);
assert.throws(()=>m.status(cap(()=>{throw Error('unexpected');})),e=>e.code==='UBI-R0004');
"#,
    );
}

#[test]
fn declared_import_effects_permit_direct_calls_and_pure_generic_callbacks() {
    let a = checked(&[
        ("host.ubi", "export fn write(c: Console,text: string) -> Result<unit,HostError> effects(console) { consoleLog(c,text) }"),
        ("main.ubi", "import { write } from \"./host.ubi\"; fn id<T>(x: T) -> T { x } export fn f(c: Console) -> Result<int,HostError> effects(console) { write(c,\"ok\")?; Result.Ok(length(map([1,2],id))) }"),
    ]);
    let c = Value::Capability(Rc::new(HostCapability::new(
        CapabilityKind::Console,
        |op, args| {
            assert_eq!(op, "consoleLog");
            assert_eq!(args, ["ok"]);
            Ok("{\"tag\":\"Ok\",\"values\":[null]}".into())
        },
    )));
    let result = run(&a, "f", vec![c]).unwrap();
    let Value::Enum { tag, values, .. } = result else {
        panic!("Result expected");
    };
    assert_eq!(&*tag, "Ok");
    assert_eq!(values.as_slice(), &[Value::Int(2)]);
    node(&a, "const host=await import('./.ubi-host.mjs'); let calls=0; const c=host.createConsoleCapability(text=>{assert.equal(text,'ok');calls++;return '{\"tag\":\"Ok\",\"values\":[null]}';}); assert.deepEqual(m.f(c),{tag:'Ok',values:[2]}); assert.equal(calls,1);");
}
