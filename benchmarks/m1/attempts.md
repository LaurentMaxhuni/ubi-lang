# M1 mini agent benchmark attempts

Run date: 2026-10-01. Independent solution agent used `tasks.md` and
`SPEC.md` core profile. Working directory: `C:\github-projects\ubi-lang`.
Existing `target/debug/ubi.exe` was available; no build was needed.
Each iteration consists of writing an attempted solution and running the
listed check. Limit: three iterations per task.

| Task | Iterations | Compilation converged | Limit failure |
| --- | ---: | --- | --- |
| Repair | 2 | Yes | No |
| Two modules | 1 | Yes | No |
| Guard division | 1 | Yes | No |
| Return early | 1 | Yes | No |

## Repair

Iteration 1 wrote the required faulty body `value + true`.

```powershell
target/debug/ubi.exe check main.ubi --root benchmarks/m1/repair --json
```

Exit status: 1. Actual diagnostic:

```json
{"code":"UBI0020","severity":"error","message":"Arithmetic operands must have the same type: int and bool","primary":{"sourceId":"main.ubi","start":53,"end":57},"related":[]}
```

Faulty source revision:
`sha256:7388a17628decdf78ebe675e7766175f7ad1d13d71d2741c6dfcd9840a86a558`.

Iteration 2 replaced `true` with integer `1` using the type diagnostic.

```powershell
target/debug/ubi.exe check main.ubi --root benchmarks/m1/repair --json
```

Exit status: 0. JSON diagnostics: `[]`. Compilation converged in 2 iterations.

## Two modules

Iteration 1 wrote exported `subtotal` using multiplication and imported it
from `./math.ubi`. `total` uses `if (quantity <= 0)` to return zero; its
`else` branch calls `subtotal` and adds fee.

```powershell
target/debug/ubi.exe check main.ubi --root benchmarks/m1/modules --json
```

Exit status: 0. JSON diagnostics: `[]`. Envelope included `main.ubi` and
`math.ubi`. Compilation converged in 1 iteration.

## Guard division

Iteration 1 wrote `divideOr` with a zero-denominator branch returning fallback
and an `else` branch using integer `/`.

```powershell
target/debug/ubi.exe check main.ubi --root benchmarks/m1/division --json
```

Exit status: 0. JSON diagnostics: `[]`. Compilation converged in 1 iteration.

## Return early

Iteration 1 wrote `label` with explicit `return "[done] " + title;` in the
completed branch, a unit-valued `else`, and trailing `"[todo] " + title`.

```powershell
target/debug/ubi.exe check main.ubi --root benchmarks/m1/labels --json
```

Exit status: 0. JSON diagnostics: `[]`. Compilation converged in 1 iteration.

## Evaluation boundary

These outcomes establish compilation convergence only. Solution agent did
not run either execution backend, grade behavior, modify compiler code, or
read `grade.rs`, `evaluate.ps1`, or `evaluation.md`. Independent evaluator
owns correctness grading. No evaluator-owned hidden corpus was provisioned.
