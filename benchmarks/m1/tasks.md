# M1 mini agent benchmark

Run before Milestone 2. Specification: `SPEC.md` core profile only.
Each task has a limit of three compile/fix iterations. One iteration means
writing an attempted solution and running `ubi check --json` on its entry.
Record diagnostics and whether compilation converged. Passing compilation
does not establish correctness; a separate evaluator grades both backends.
No evaluator-owned hidden corpus is provisioned by this benchmark.

## Tasks

1. **Repair a type error.** In `repair/main.ubi`, implement
   `export fn increment(value: int) -> int { value + 1 }` starting from
   the faulty body `value + true`. First check the faulty source, then
   fix it using JSON diagnostics. Preserve checked integer overflow.
2. **Use two modules.** In `modules/math.ubi`, export
   `subtotal(quantity: int, price: int) -> int`, multiplying quantity by
   price. In `modules/main.ubi`, import it and export
   `total(quantity: int, price: int, fee: int) -> int`. Return zero when
   quantity is nonpositive, without evaluating subtotal or adding fee;
   otherwise return subtotal plus fee. Preserve checked arithmetic.
3. **Guard division.** In `division/main.ubi`, export
   `divideOr(numerator: int, denominator: int, fallback: int) -> int`.
   Return fallback for a zero denominator; otherwise use integer division.
   Preserve signed truncation and specified overflow faults.
4. **Return early.** In `labels/main.ubi`, export
   `label(title: string, completed: bool) -> string`. For completed tasks,
   return `"[done] " + title` through an explicit early return inside an
   `if` branch. Otherwise return `"[todo] " + title`. Preserve title bytes,
   including Unicode and empty strings; use no Milestone 2 features.

Solution agent owns only the four task directories and `attempts.md`.
Evaluator owns `grade.rs`, `evaluate.ps1`, and `evaluation.md`, and derives
expected values/faults independently from these contracts and SPEC.md.
Implementer must not read evaluator artifacts or grade its own solutions.

Run the independent public evaluator with `powershell -NoProfile -File
benchmarks/m1/evaluate.ps1` from the repository, or `cargo test --lib
benchmark_grade`. The evaluator joins the normal test suite through a
test-only module; no production interpreter API is exposed.
