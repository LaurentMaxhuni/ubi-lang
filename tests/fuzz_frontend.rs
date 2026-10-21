use ubi_lang::Compiler;

const ALPHABET: &[char] = &[
    'a', 'z', '0', '9', ' ', '\n', '\t', '(', ')', '{', '}', '[', ']', '<', '>', '=', '!', '+',
    '-', '*', '/', '%', '&', '|', ':', ';', ',', '.', '?', '"', '\'', '_', 'λ', '💩',
];

#[test]
fn deterministic_mutations_never_crash_the_parser_or_checker() {
    let seeds = [
        "export fn f(x: int) -> int { let y = x + 1; if y > 2 { return y } else { 0 } }",
        "import { value } from \"./lib.ubi\"; export fn main() -> string { value() }",
        "fn recurse(n: int) -> int { if n == 0 { 0 } else { recurse(n - 1) } }",
        "export fn choose(flag: bool) -> float { if flag { 0.0 } else { -0.0 } }",
        "record Box { callback: fn(int) -> int } fn make(n: int) -> fn(int) -> int { (x: int) => x + n } export fn f() -> int { let box = Box { callback: make(3) }; let xs: List<fn(int) -> int> = [box.callback]; match (xs[0]) { Option.Some(f) => f(4), Option.None => 0 } }",
        "record Point { x: int, y: int } export fn f() -> int { let p = Point { y: 2, x: 1 }; let q = Point { ...p, x: 7, }; q.x + p.y }",
        "export fn f(x: int) -> int { let mut y = x; y = { let mut y = y + 1; y = y + 2; y }; if (y > 2) { return y; } else { y = 0; }; y }",
        "record Inner { value: float } record Outer { inner: Inner } export fn f() -> bool { let p = Outer { inner: Inner { value: 0.0 / 0.0 } }; let alias = p; p == alias }",
        "export record P { __proto__: int, done: unit } export fn f(p: P) -> P { let mut result: P = p; result = P { ...result, __proto__: result.__proto__ + 1 }; result }",
        "export fn f(xs: List<Option<int>>) -> int { let mut total = 0; for (item in xs) { total = total + match (item) { Option.Some(n) if (n > 0) => n, Option.Some(n) => 0, Option.None => 0 }; } total }",
        "export fn f() -> int { let mut n = 1; let cb = (x: int) => { let mut y = x + n; while (y < 4) { y = y + 1; } y }; n = 9; fold(map([1, 2], cb), 0, (a: int, b: int) => a + b) }",
        "export fn f() -> string { join(filter(split(\"a😀b\", \"\"), (s: string) => contains(\"a😀\", s)), \"|\") }",
        "export fn f() -> int { let length = (n: int) => n + 1; let trim = (n: int) => length(n); fold(map([1, 2], trim), 0, (a: int, b: int) => a + b) }",
        "enum Flags { Empty, Pair(bool, Option<bool>) } export fn f() -> int { match (Flags.Pair(true, Option.Some(false))) { Flags.Empty => 0, Flags.Pair(_, Option.None) => 1, Flags.Pair(true, Option.Some(_)) => 2, Flags.Pair(false, Option.Some(_)) => 3 } }",
        "enum Chain { End, Next(int, Chain) } fn sum(c: Chain) -> int { match (c) { Chain.End => 0, Chain.Next(n, tail) => n + sum(tail) } } export fn f() -> int { sum(Chain.Next(1, Chain.Next(2, Chain.End))) }",
        "enum E { Empty, Value(int) } fn make() -> E { E.Value(4) } export fn f() -> int { let E = 7; match (make()) { E.Empty => E, E.Value(n) => E + n } }",
        "record Box<T> { value: T } fn id<T>(x: T) -> T { x } fn empty<T>() -> List<T> { [] } export fn f() -> int { let box: Box<List<int>> = Box { value: id(empty()) }; length(box.value) + length(map([1,2],id)) }",
        "enum Tree<T> { Leaf(T), Branch(Tree<T>, Tree<T>) } fn total(t: Tree<int>) -> int { match (t) { Tree.Leaf(n) => n, Tree.Branch(a,b) => total(a) + total(b) } } export fn f() -> int { total(Tree.Branch(Tree.Leaf(2),Tree.Leaf(3))) }",
        "fn pass<T,E>(r: Result<T,E>) -> Result<T,E> { Result.Ok(r?) } export fn f(b: bool) -> Result<List<int>,string> { let r: Result<int,string> = if (b) { Result.Ok(3) } else { Result.Err(\"stop\") }; let n = pass(r)?; Result.Ok([n]) }",
        "export fn f() -> bool { let empty: Map<string,Option<List<int>>> = mapEmpty(); let one = mapSet(empty,\"__proto__\",Option.Some([])); let two = mapSet(one,\"constructor\",Option.None); mapGet(two,\"__proto__\") == Option.Some(Option.Some([])) && mapKeys(mapRemove(two,\"constructor\")) == [\"__proto__\"] }",
        "fn pair<T>(value: T) -> (T, T) { (value, value) } export fn f() -> int { let (first, _) = pair(7); let singleton = (first,); let (answer,) = singleton; answer }",
        "fn pair<T>(value: T) -> (T, T) { (value, value) } fn discard<T>(value: T) -> unit { () } fn build<T>(seed: T) -> int { let a1 = pair(seed); let a2 = pair(a1); let a3 = pair(a2); let a4 = pair(a3); let a5 = pair(a4); let a6 = pair(a5); let a7 = pair(a6); let a8 = pair(a7); let (d8, _) = a8; let (d7, _) = d8; let (d6, _) = d7; let (d5, _) = d6; let (d4, _) = d5; let (d3, _) = d4; let (result, _) = d3; discard(result); 1 } export fn f() -> int { build(7) }",
        "fn write(c: Console,text: string) -> Result<unit,HostError> effects(console) { consoleLog(c,text) } export fn f(c: Console,s: Storage) -> Result<Option<string>,HostError> effects(console,storage) { let alias = c; write(alias,\"hello😀\")?; storageSet(s,\"key\",\"value\")?; let found = storageGet(s,\"key\")?; storageRemove(s,\"key\")?; Result.Ok(found) }",
        "fn plain(x: int) -> int { x + 1 } fn generic<T>(x: T) -> int { let capture = () => x; 2 } export fn f(b: bool) -> int { (if (b) { plain } else { generic })(5) }",
        "async fn value(n: int) -> int { n } export async fn f() -> int { let mut n = 0; for (x in [1,2,3]) { n = n + await value(x); } n }",
        "export async fn f(h: Http,s: Storage) -> Result<string,HostError> effects(http,storage) { let text = (await httpGet(h,\"url\"))?; (await storageSetAsync(s,\"key\",text))?; Result.Ok(text) }",
    ];
    for seed in &seeds[4..] {
        let mut compiler = Compiler::new();
        compiler
            .add_source("fuzz.ubi", seed.as_bytes().to_vec())
            .unwrap();
        let build = compiler.build();
        assert!(
            build.diagnostics.is_empty(),
            "invalid M2 seed: {seed}: {:?}",
            build.diagnostics
        );
        assert!(build.javascript.is_some());
    }
    let mut state = 0x9e37_79b9_7f4a_7c15u64;

    for case in 0..10_000 {
        let seed: Vec<char> = seeds[case % seeds.len()].chars().collect();
        let mut input = seed;
        let edits = (next(&mut state) % 5 + 1) as usize;
        for _ in 0..edits {
            let index = (next(&mut state) as usize) % (input.len() + 1);
            match next(&mut state) % 3 {
                0 if index < input.len() => {
                    input.remove(index);
                }
                1 if index < input.len() => {
                    input[index] = ALPHABET[(next(&mut state) as usize) % ALPHABET.len()];
                }
                _ => input.insert(
                    index,
                    ALPHABET[(next(&mut state) as usize) % ALPHABET.len()],
                ),
            }
        }

        let mut compiler = Compiler::new();
        compiler
            .add_source(
                "fuzz.ubi",
                input.into_iter().collect::<String>().into_bytes(),
            )
            .unwrap();
        let check = compiler.check();
        if check.diagnostics.is_empty() {
            let build = compiler.build();
            assert!(
                build.diagnostics.is_empty(),
                "checked mutation {case} failed to build: {:?}",
                build.diagnostics
            );
            assert!(
                build.javascript.is_some(),
                "checked mutation {case} produced no JavaScript"
            );
        }
    }
}

fn next(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

#[test]
fn growing_nominal_substitutions_report_resource_exhaustion() {
    for source in [
        "record Grow<T> { child: Option<Grow<List<Option<T>>>> } export fn f(x: Grow<int>) -> int { 0 }",
        "enum Grow<T> { End, Next(Grow<Option<List<T>>>) } export fn f(x: Grow<int>) -> int { 0 }",
        "record First<T> { child: Option<Second<List<T>>> } enum Second<U> { End, Next(First<U>) } export fn f(x: First<int>) -> int { 0 }",
        "fn expand<T>(x: T) -> int { expand([x]) } export fn f() -> int { expand(1) }",
        "fn expand<T>(x: T) -> int { expand((x, x)) } export fn f() -> int { expand(1) }",
    ] {
        let mut compiler = Compiler::new();
        compiler.add_source("resource.ubi", source.as_bytes().to_vec()).unwrap();
        let checked = compiler.check();
        assert!(checked.diagnostics.iter().any(|d|d.code=="UBI0090"), "{source}: {:?}", checked.diagnostics);
        assert!(compiler.build().javascript.is_none());
    }
    // Recursive function calls preserve T and do not expand nominal definitions.
    let mut compiler = Compiler::new();
    compiler.add_source("resource.ubi", b"fn recurse<T>(x: T,n: int) -> T { if (n == 0) { x } else { recurse(x,n - 1) } } export fn f() -> int { recurse(7,3) }".to_vec()).unwrap();
    let built = compiler.build();
    assert!(built.diagnostics.is_empty(), "{:?}", built.diagnostics);
    assert!(built.javascript.is_some());
}
