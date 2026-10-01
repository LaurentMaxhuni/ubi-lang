# M1 public independent evaluation

Oracle derived from `tasks.md` and SPEC.md before reading solver solutions.
This is public independent evaluation, not an evaluator-owned hidden corpus.
Compilation and backend agreement alone are insufficient: each backend must
match explicit independent expected values/faults.

Run from repository root, with Cargo and Node on PATH:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File benchmarks/m1/evaluate.ps1
```

Optional Node override: `-NodePath C:\nvm4w\nodejs\node.exe`.
Harness uses a permanent test-only module hook in `src/lib.rs`, so benchmark
grading also runs with the ordinary library tests. Evaluating does not modify
source files. No production compiler changes are required.
Rust stdlib and Node built-ins only; generated modules use a temporary directory
cleaned when the test exits. Missing Node or any test failure fails evaluation.

## Independent cases

45 cases per backend, 90 runtime expectations total, plus one AST check that
`label` contains an explicit return inside an if branch. `grade.rs` contains all
inputs and exact expected values/fault codes.

| Export | Cases | Coverage |
| --- | ---: | --- |
| increment | 6 | zero, negative, ordinary, both int bounds, maximum overflow |
| total | 12 | positive quantity, negative/zero prices, bounds, zero/nonpositive bypass despite arithmetic that would fault, multiply overflow before fee, fee overflow |
| subtotal | 6 | positive/negative/zero multiplication, both bounds, minimum times -1 fault |
| divideOr | 13 | four sign combinations, truncation toward zero, boundaries, minimum/-1 overflow, three zero-divisor fallbacks |
| label | 8 | both completed states with empty/ASCII/non-BMP/CJK/decomposed Unicode and newline/tab/quote/backslash/NUL titles |

String results compare exact scalar sequences without normalization. The
interpreter uses Rust strings; generated JS uses independently escaped scalar
literals and Node strict equality. Faults require the expected stable code and
a nonempty message. The zero denominator contract returns fallback, so no
division-by-zero fault is expected from `divideOr`.

## Results

2026-10-01: final command
`powershell -NoProfile -ExecutionPolicy Bypass -File benchmarks/m1/evaluate.ps1`
passed: 3 tests, 0 failures. AST interpreter: 45/45. Generated JavaScript:
45/45. Explicit early return AST check passed. No solution changes requested.

`rustfmt --edition 2021 --check benchmarks/m1/grade.rs` and `git diff --check`
also passed. Environment: Node v22.22.1; Cargo 1.97.1. Evaluated working tree
based on Git revision `03d0a34a033279b637deed7c7125bc90210aacc0`; benchmark
artifacts and test hook were uncommitted at evaluation time.

Initial evaluator run passed AST 45/45 and the structural check, but reported
43/45 JavaScript results because Node strict equality distinguishes -0 from 0
for `divideOr(-1, 2, 99)` and `divideOr(0, -1, 99)`. That comparator was overly
strict for Ubi int: SPEC section 4 defines signed zero behavior for floats,
while int zero has no observable sign. Evaluator corrected integer checks to
require `Number.isInteger`, i32 range, and numeric equality (`===`). String
strict equality and stable fault checks remain unchanged. Final run passed all
expectations without modifying compiler or solver sources.

Scope: public inputs cover the four specified tasks and their arithmetic and
text boundaries. No hidden corpus was provisioned or run; this evaluation does
not establish complete compiler conformance.
