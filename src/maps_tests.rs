//! Independently authored conformance from SPEC section 19.
use crate::generics_tests::{checked, node, rejected, run};
use crate::interpreter::{InvocationError, Value};

#[test]
fn immutable_maps_keep_order_aliases_and_order_independent_equality() {
    let a = checked(&[(
        "main.ubi",
        r#"
export fn ordered() -> bool {
  let empty: Map<string,int> = mapEmpty();
  let a = mapSet(mapSet(empty,"b",2),"a",1); let alias = a;
  let b = mapSet(a,"b",7); let c = mapSet(mapRemove(b,"b"),"b",8);
  let reverse = mapSet(mapSet(empty,"a",1),"b",2);
  length(empty) == 0 && length(a) == 2 && a == alias && a == reverse
    && mapKeys(a) == ["b","a"] && mapValues(a) == [2,1]
    && mapKeys(b) == ["b","a"] && mapValues(b) == [7,1]
    && mapKeys(c) == ["a","b"] && mapValues(c) == [1,8]
    && mapGet(a,"b") == Option.Some(2) && mapGet(a,"x") == Option.None
    && mapHas(a,"b") && !mapHas(a,"x") && mapRemove(a,"x") == a
    && b != a && mapRemove(a,"b") != a
}
export fn keyKinds() -> bool {
  let ints: Map<int,string> = mapEmpty(); let bools: Map<bool,int> = mapEmpty();
  let numbers = mapSet(mapSet(ints,-2147483648,"min"),2147483647,"max");
  let flags = mapSet(mapSet(bools,true,1),false,2);
  let wordBase: Map<string,int> = mapEmpty();
  let words = mapSet(mapSet(mapSet(wordBase,"__proto__",3),"constructor",4),"😀",5);
  mapKeys(numbers) == [-2147483648,2147483647] && mapGet(numbers,2147483647) == Option.Some("max")
    && mapKeys(flags) == [true,false] && mapValues(flags) == [1,2]
    && mapGet(words,"__proto__") == Option.Some(3)
    && mapGet(words,"constructor") == Option.Some(4) && mapGet(words,"😀") == Option.Some(5)
}
export fn iteration() -> int {
  let empty: Map<int,int> = mapEmpty(); let m = mapSet(mapSet(empty,7,10),3,20);
  let mut n = 0; for (key in mapKeys(m)) { n = n + key; }
  for (value in mapValues(m)) { n = n + value; } n
}
"#,
    )]);
    assert_eq!(run(&a, "ordered", vec![]), Ok(Value::Bool(true)));
    assert_eq!(run(&a, "keyKinds", vec![]), Ok(Value::Bool(true)));
    assert_eq!(run(&a, "iteration", vec![]), Ok(Value::Int(40)));
    node(&a, "assert.equal(m.ordered(),true); assert.equal(m.keyKinds(),true); assert.equal(m.iteration(),40);");
}

#[test]
fn map_arguments_execute_once_left_to_right_and_stop_on_first_fault() {
    let a = checked(&[(
        "main.ubi",
        r#"
export fn order() -> int {
  let mut n = 0; let old: Map<string,int> = mapEmpty();
  let x = mapSet({ n = n + 1; old }, { n = n + 1; toString(n) }, { n = n + 1; n });
  n * 100 + match (mapGet(x,"2")) { Option.Some(v) => v, Option.None => 0 }
}
export fn firstFault() -> Map<int,int> {
  let old: Map<int,int> = mapEmpty(); mapSet(old,1 / 0,2147483647 + 1)
}
export fn secondFault() -> Map<int,int> {
  let old: Map<int,int> = mapEmpty(); mapSet(old,2147483647 + 1,1 / 0)
}
export fn nan() -> bool {
  let old: Map<string,float> = mapEmpty(); let x = mapSet(old,"n",0.0 / 0.0); x == x
}
export fn zeros() -> bool {
  let old: Map<string,float> = mapEmpty(); mapSet(old,"n",0.0) == mapSet(old,"n",-0.0)
}
"#,
    )]);
    assert_eq!(run(&a, "order", vec![]), Ok(Value::Int(303)));
    assert_eq!(run(&a, "nan", vec![]), Ok(Value::Bool(false)));
    assert_eq!(run(&a, "zeros", vec![]), Ok(Value::Bool(true)));
    for (name, code) in [("firstFault", "UBI-R0002"), ("secondFault", "UBI-R0001")] {
        let Err(InvocationError::Runtime(fault)) = run(&a, name, vec![]) else {
            panic!("expected {code}");
        };
        assert_eq!(fault.code, code);
    }
    node(&a, "assert.equal(m.order(),303); assert.equal(m.nan(),false); assert.equal(m.zeros(),true); assert.throws(()=>m.firstFault(),e=>e.code==='UBI-R0002'); assert.throws(()=>m.secondFault(),e=>e.code==='UBI-R0001');");
}

#[test]
fn map_values_and_expected_types_compose_with_generics() {
    let a = checked(&[(
        "main.ubi",
        r#"
record Box<T> { value: T }
fn id<T>(x: T) -> T { x }
fn empty<V>() -> Map<string,V> { mapEmpty() }
fn put<V>(m: Map<string,V>, x: V) -> Map<string,V> { mapSet(m,"x",x) }
fn consume(m: Map<string,List<int>>) -> int { length(m) }
export fn contexts() -> bool {
  let a: Map<string,List<int>> = id(mapEmpty());
  let b: Map<string,List<int>> = put(empty(),[]);
  let c: Map<string,Option<int>> = mapSet(mapEmpty(),"a",Option.None);
  let d: Map<string,Box<List<int>>> = mapSet(mapEmpty(),"box",Box { value: [] });
  length(a) == 0 && consume(mapEmpty()) == 0 && mapGet(b,"x") == Option.Some([])
    && mapGet(c,"a") == Option.Some(Option.None)
    && mapGet(d,"box") == Option.Some(Box { value: [] })
}
"#,
    )]);
    assert_eq!(run(&a, "contexts", vec![]), Ok(Value::Bool(true)));
    node(&a, "assert.equal(m.contexts(),true);");
}

#[test]
fn invalid_map_keys_values_invariance_and_iteration_reject() {
    for source in [
        "fn f(x: Map<float,int>) {}",
        "fn f(x: Map<List<int>,int>) {}",
        "record R {} fn f(x: Map<R,int>) {}",
        "fn f<K,V>(x: Map<K,V>) -> int { length(x) }",
        "fn f() { mapEmpty(); }",
        "fn f(x: Map<int,int>) { mapSet(x,true,1); }",
        "fn f(x: Map<int,int>) { mapSet(x,1,true); }",
        "fn f(x: Map<int,int>) { mapGet(x,true); }",
        "fn f(x: Map<int,int>) { mapHas(x,\"1\"); }",
        "fn f(x: Map<int,int>) { mapRemove(x,1.0); }",
        "fn f(x: Map<int,int>) { for (n in x) {} }",
        "fn f(x: Map<int,int>) -> Map<int,bool> { x }",
        "fn f(x: Map<int>) {}",
        "fn f() { let x: Map<int,int> = mapEmpty(); mapSet(x,1,(n: int) => n); }",
        "fn equal<V>(m: Map<string,V>) -> bool { m == m }",
    ] {
        rejected(&[("main.ubi", source)], "UBI0020");
    }
    rejected(&[("main.ubi", "fn mapEmpty() {}")], "UBI0011");
    rejected(&[("main.ubi", "record Map {}")], "UBI0011");
}

#[test]
fn map_host_entry_arrays_validate_exactly_and_freeze_recursively() {
    let a = checked(&[(
        "main.ubi",
        r#"
export record Item { value: int }
export fn echo(x: Map<string,List<Option<Item>>>) -> Map<string,List<Option<Item>>> { x }
export fn ints(x: Map<int,string>) -> List<int> { mapKeys(x) }
export fn flags(x: Map<bool,int>) -> List<int> { mapValues(x) }
export fn make() -> Map<string,List<Option<Item>>> {
  let m: Map<string,List<Option<Item>>> = mapEmpty();
  mapSet(mapSet(m,"__proto__",[Option.Some(Item { value: 7 })]),"constructor",[])
}

"#,
    )]);
    let v = run(&a, "make", vec![]).unwrap();
    assert_eq!(run(&a, "echo", vec![v.clone()]), Ok(v));
    assert_eq!(
        run(&a, "echo", vec![Value::Int(0)]),
        Err(InvocationError::InvalidArguments)
    );
    node(
        &a,
        r#"
const json='[["__proto__",[{"tag":"Some","value":{"value":7}}]],["constructor",[]]]';
const out=m.echo(json); assert.deepEqual(out,m.make());
for(const x of [out,out[0],out[0][1],out[0][1][0],out[0][1][0].value]) assert.ok(Object.isFrozen(x));
assert.deepEqual(m.ints('[[-2147483648,"a"],[2147483647,"b"]]'),[-2147483648,2147483647]);
assert.deepEqual(m.flags('[[true,1],[false,2]]'),[1,2]);
for(const bad of [out,'{}','null','[[]]','[["x"]]','[["x",[],0]]','[["x",[]],["x",[]]]','[[1,[]]]','[["x",[{"tag":"Some","value":{"value":false}}]]]','{"entries":[]}']) assert.throws(()=>m.echo(bad),TypeError);
for(const bad of ['[[0,"a"],[-0,"b"]]','[[2147483648,"x"]]','[[true,"x"]]']) assert.throws(()=>m.ints(bad),TypeError);
assert.throws(()=>m.flags('[[true,1],[true,2]]'),TypeError);
let reads=0; const live={get entries(){reads++; throw Error('getter');}};
assert.throws(()=>m.echo(live),TypeError); assert.equal(reads,0);
"#,
    );
}

#[test]
fn map_depth_counts_value_children_and_excludes_entry_envelopes() {
    let a = checked(&[(
        "main.ubi",
        r#"
export enum Chain { End, Next(Chain) }
export fn echo(x: Map<string,Chain>) -> Map<string,Chain> { x }
export fn make() -> Map<string,Chain> {
  let mut child = Chain.End;
  for (n in range(0,30)) { child = Chain.Next(child); }
  mapSet(mapEmpty(),"x",child)
}
export fn excessive() -> Map<string,Chain> {
  let mut child = Chain.End;
  for (n in range(0,31)) { child = Chain.Next(child); }
  mapSet(mapEmpty(),"x",child)
}
"#,
    )]);
    let legal_value = run(&a, "make", vec![]).unwrap();
    assert_eq!(run(&a, "echo", vec![legal_value.clone()]), Ok(legal_value));
    let Err(InvocationError::Runtime(fault)) = run(&a, "excessive", vec![]) else {
        panic!("map depth overflow accepted");
    };
    assert_eq!(fault.code, "UBI-R0005");
    node(&a, "let value={tag:'End',values:[]}; for(let i=0;i<30;i++) value={tag:'Next',values:[value]}; const legal=[['x',value]]; assert.deepEqual(m.echo(JSON.stringify(legal)),m.make()); value={tag:'Next',values:[value]}; assert.throws(()=>m.echo(JSON.stringify([['x',value]])),TypeError); assert.throws(()=>m.excessive(),e=>e.code==='UBI-R0005');");
}
