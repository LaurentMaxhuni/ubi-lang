use ubi_lang::Compiler;

const ALPHABET: &[char] = &[
    'a', 'z', '0', '9', ' ', '\n', '\t', '(', ')', '{', '}', '[', ']', '<', '>', '=', '!', '+',
    '-', '*', '/', '%', '&', '|', ':', ';', ',', '.', '"', '\'', '_', 'λ', '💩',
];

#[test]
fn deterministic_mutations_never_crash_the_parser_or_checker() {
    let seeds = [
        "export fn f(x: int) -> int { let y = x + 1; if y > 2 { return y } else { 0 } }",
        "import { value } from \"./lib.ubi\"; export fn main() -> string { value() }",
        "fn recurse(n: int) -> int { if n == 0 { 0 } else { recurse(n - 1) } }",
        "export fn choose(flag: bool) -> float { if flag { 0.0 } else { -0.0 } }",
        "record Point { x: int, y: int } export fn f() -> int { let p = Point { y: 2, x: 1 }; let q = Point { ...p, x: 7, }; q.x + p.y }",
        "export fn f(x: int) -> int { let mut y = x; y = { let mut y = y + 1; y = y + 2; y }; if (y > 2) { return y; } else { y = 0; }; y }",
        "record Inner { value: float } record Outer { inner: Inner } export fn f() -> bool { let p = Outer { inner: Inner { value: 0.0 / 0.0 } }; let alias = p; p == alias }",
        "export record P { __proto__: int, done: unit } export fn f(p: P) -> P { let mut result: P = p; result = P { ...result, __proto__: result.__proto__ + 1 }; result }",
        "export fn f(xs: List<Option<int>>) -> int { let mut total = 0; for (item in xs) { total = total + match (item) { Option.Some(n) if (n > 0) => n, Option.Some(n) => 0, Option.None => 0 }; } total }",
        "export fn f() -> int { let mut n = 1; let cb = (x: int) => { let mut y = x + n; while (y < 4) { y = y + 1; } y }; n = 9; fold(map([1, 2], cb), 0, (a: int, b: int) => a + b) }",
        "export fn f() -> string { join(filter(split(\"a😀b\", \"\"), (s: string) => contains(\"a😀\", s)), \"|\") }",
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
            if build.diagnostics.is_empty() {
                assert!(build.javascript.is_some());
            } else {
                assert!(build.javascript.is_none());
            }
        }
    }
}

fn next(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}
