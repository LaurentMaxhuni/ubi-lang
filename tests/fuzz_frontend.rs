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
    ];
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
