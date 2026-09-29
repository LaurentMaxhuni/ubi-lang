# Ubi implementation plan

Status: Milestone 0 gate approved for reviewed revision `0dd23e85f93fffcd2657f3c24d944c1197fd24e3`. Milestone 1 compiler implementation in progress.

Ubi (`.ubi`, short for ubiquitous) combines familiar JavaScript-style syntax with strict types, explicit errors, and checked access to host capabilities. AI agents write code; humans must be able to understand and review it.

## Product target

Support websites, CLI tools, desktop applications, and mobile applications from one project. Share application logic and explicit platform interfaces. Develop shared UI and platform integrations as a separate layer on that foundation.

The first reference application is a task list: create tasks, validate titles, complete tasks, filter tasks, and persist them. Its domain logic lives in the same Ubi modules on every platform. Initial interfaces and packaging use thin JavaScript hosts.

A working task list proves portability; production readiness additionally requires lifecycle handling, accessibility, packaging, diagnostics, testing, and reliable platform integrations.

## Starting decisions

These defaults guide implementation. Record revisions in the language specification before changing behavior.

| Area | Initial decision |
| --- | --- |
| Compiler | Rust library with a thin CLI; source input and output artifacts independent of filesystem access |
| Output | JavaScript ES modules; use host garbage collection |
| Bindings | `let` is immutable; `let mut` permits local reassignment |
| Data | Immutable records and collections; explicit updates produce new values |
| Functions | `fn`, explicit parameter types, local inference; exported return types required |
| Types | Nominal records and enums; basic parametric generics; no implicit coercion |
| Errors | `Option<T>`, `Result<T, E>`, exhaustive `match`; `?` initially requires the same error type |
| Modules | Explicit named imports and exports; reject module cycles initially |
| Permissions | Declared effects checked through calls; explicit host-supplied capabilities |
| Foreign code | Trusted adapters validate incoming data and normalize expected failures |
| Tooling | Stable diagnostic codes and versioned JSON output from the first compiler slice |

Do not expose an unrestricted `any`, unchecked JSON casts, ambient host globals, or shared mutable objects in the initial language. Fatal runtime faults remain possible and must have specified behavior; `Result` covers recoverable failures.

## Agent workflow and independent review

- Required repository documents: `SPEC.md` defines behavior, `DECISIONS.md` records reasoning and tradeoffs, and `PLAN.md` tracks milestones and their gates. Milestone 0 creates the missing specification and decisions log alongside the example corpus.
- Every session starts by reading all three documents and stating the current milestone. During bootstrap, explicitly report missing documents; do not invent their contents. Current milestone: Milestone 1.
- Before implementing any behavior change, update `SPEC.md` first and log the reasoning in `DECISIONS.md`.
- Make small commits, one meaningful change per commit.
- The conformance corpus must be authored or reviewed separately from implementation work, against the specification. An implementation session must not write and grade its own tests; route new cases and expected results through an independent reviewer/evaluator.
- Maintain a hidden test set outside the implementation session's accessible workspace and credentials. A human or separate evaluator owns and runs it; implementation sessions cannot see or edit its contents or change the grading rules. Supply evaluation outcomes without exposing hidden cases.
- Independent evaluation checks specified expected behavior as well as interpreter/JavaScript agreement: two implementations agreeing on the wrong result is still a failure.
- Every milestone requires human sign-off in `DECISIONS.md`, identifying the reviewed revision and evidence. Passing automated checks alone does not pass a completion gate. Changes that invalidate reviewed evidence require renewed sign-off.

## Milestone 0 — Specify observable behavior

Progress: complete for reviewed revision `0dd23e85f93fffcd2657f3c24d944c1197fd24e3`; independently reviewed artifacts and human acceptance recorded in `DECISIONS.md`.

- Write a compact `SPEC.md` covering grammar, bindings, values, functions, modules, matching, errors, and evaluation order.
- Define `int` width and overflow behavior, float behavior, division, equality, string indexing, and out-of-bounds collection access. Prefer signed 32-bit integers initially; require consistent behavior across backends.
- Define immutable record update semantics and distinguish reassignment from mutation.
- Decide the initial immutable collection representation explicitly: plain copying for v0, with persistent data structures deferred until profiling justifies them. Record this choice and its costs in `SPEC.md` and `DECISIONS.md`.
- Specify `?` conversion explicitly: same error type only initially, with explicit mapping for different errors. Revisit From-style conversion in Milestone 2 through a recorded decision; no implicit conversion is assumed.
- Define foreign-boundary validation, fatal faults, and unsupported operations.
- Create small valid/invalid examples, including the task-list module's intended interface.
- Specify diagnostic schema: version, code, severity, message, primary span, related spans, optional edits, and edit applicability. Use explicit offset units and source revision identity for edits.

Completion gate: representative programs have unambiguous expected values or diagnostics. Grammar and examples agree. No implementation depends on unspecified JavaScript behavior.

What the human checks:

- Approve the specification, example expectations, collection representation, and error-conversion decision.
- Approve `DECISIONS.md` and sign off on the completion gate.

## Milestone 1 — Compile one complete program

Progress: source/span management, lexer, recursive-descent/Pratt parser, import graph resolver, primitive name/type checker, typed IR lowering, independent AST interpreter, and deterministic JavaScript ES module backend implemented in one Rust package. Forty-seven tests pass; CLI behavior, browser execution, and fuzzing remain.

Build a vertical slice before expanding the language:

1. Create one Rust package with library and CLI modules; split crates only when needed.
2. Add source management, lexer, and spans.
3. Add a recursive-descent parser with Pratt expression parsing; retain comments for later formatting.
4. Resolve names and check concrete function signatures and expressions.
5. Lower checked code to a small typed representation.
6. Emit readable JavaScript ES modules.
7. Implement `ubi check` and `ubi build`, including JSON diagnostics.
8. Add a small tree-walking reference interpreter, implemented separately from JavaScript code generation. Evaluate source-level syntax without using codegen lowering or generated JavaScript as the interpreter's implementation.
9. Start parser and type-checker fuzzing. Random or malformed input must produce diagnostics, never compiler crashes or hangs; enforce input/resource limits with diagnostics and use an external watchdog to detect hangs as failures. Valid generated programs may succeed normally.

Scope: literals, immutable bindings, arithmetic, named functions, calls, `if`, and two-file imports. Keep generated output deterministic for identical inputs.

Every conformance program must go through both the interpreter and compiled-JavaScript paths. Valid programs execute on both and must agree on observable behavior and specified expected results. Invalid programs must be rejected before execution on both paths. Any disagreement is a failure. Extend both implementations as features land; use identical deterministic host fixtures where needed. Shared frontend checks still require independent negative tests.

Completion gate: the same generated module computes the same result in Node and a browser. Unknown names and type mismatches fail with accurate spans and nonzero CLI exit status.

Additional gate requirements: independent conformance evaluation passes on both execution paths, with no disagreements; parser/type-checker fuzzing reports no unresolved crashes or hangs.

What the human checks:

- Run the two-host demo and skim JSON diagnostics for invalid programs.
- Review interpreter independence, conformance results, fuzzing evidence, and the decisions log; sign off on the completion gate.

After Milestone 1 and before Milestone 2, run a mini agent benchmark. Give an agent 3–4 small tasks within the implemented subset, the compact spec, and JSON diagnostics. Have an independent evaluator grade correctness. Record task outcomes, whether each compile/fix loop converges, iterations to convergence, and failures at a preset iteration limit. Human reviews the results and any resulting decisions. Keep the full Milestone 6 benchmark.

## Milestone 2 — Make the language useful for shared logic

- Add nominal records, field access, record updates, lists, and local reassignment.
- Add data enums, pattern bindings, and exhaustive matching, including nested patterns. Guards must not incorrectly establish coverage.
- Add basic generic types/functions, then `Option`, `Result`, and error propagation.
- Record the review of `?` conversion; retain same-error-only propagation unless a specification change explicitly introduces From-style conversion.
- Add closures and arrow syntax for pure `map`/`filter` callbacks. Define captures and their interaction with local reassignment; closures cannot perform effects, including indirect calls to effectful functions. Defer effect polymorphism.
- Add the minimum iteration and collection operations needed by the task list.
- Implement the task-list domain entirely in Ubi with no host access.
- Check argument evaluation order and ensure lowering never duplicates side effects.
- Extend interpreter/JavaScript conformance and parser/type-checker fuzzing to every new construct.

Completion gate: create, complete, validate, and filter tasks in both hosts using identical generated logic. Missing enum cases, invalid fields, mismatched errors, and forbidden mutation produce compile errors.

What the human checks:

- Run the task-list logic, including pure arrow callbacks in `map`/`filter`, and review invalid-program diagnostics.
- Review independent conformance/fuzzing results, capture semantics, and the error-conversion decision; approve the log and sign off.

## Milestone 3 — Establish host access and effects

- Define a small typed adapter format for JavaScript imports. Keep adapter implementation visibly outside ordinary Ubi code.
- Introduce opaque capability values that ordinary Ubi code cannot construct.
- Add finite effect sets to named function signatures and validate their propagation through the call graph, including recursion.
- Start with storage and console capabilities; add networking when async support exists.
- Restrict effectful calls to named functions initially. Defer arbitrary effectful callbacks and effect polymorphism.
- Validate foreign values; convert expected exceptions/failures into typed errors. Specify treatment of unexpected adapter faults.
- Test with fake capabilities for deterministic behavior and failure injection.
- Add an independently authored or reviewed "effect escape" suite attempting to smuggle permissions through recursion, callbacks, async, shadowed names, and re-exports. Include permitted control programs so blanket rejection cannot satisfy the suite. Unsupported constructs, including async at this stage, must be rejected explicitly; extend those cases to semantic escape attempts when the constructs become supported.

Completion gate: a program cannot access a host operation without the required capability or omit a propagated effect. Malformed foreign data becomes a decoding error. Documentation identifies trusted adapters as a security boundary.

Additional gate requirement: every effect escape attempt fails to compile; permitted controls compile. Preserve this requirement in later milestones as features expand.

What the human checks:

- Skim capability/effect diagnostics and independently evaluated escape-suite results, including permitted controls.
- Review adapter trust boundaries and the decisions log; sign off on the completion gate.

Effect declarations describe possible operations; scoped capability values control available authority. This stage does not promise isolation from malicious JavaScript dependencies.

## Milestone 4 — Add asynchronous application behavior

- Specify task creation, execution timing, awaiting, cancellation, and unhandled failure before implementing `async`/`await`.
- Ensure effects remain tracked when asynchronous work is created, passed, and awaited; deferring execution must not hide permissions.
- Add asynchronous storage and HTTP adapters returning explicit `Result` values.
- Decode JSON using explicit typed decoders initially. Do not treat a generic type parameter as validation.
- Define an explicit entrypoint with host-provided capabilities and asynchronous completion.
- Extend the effect escape suite to actual async creation, passing, and awaiting, including recursion/callback/re-export combinations. Require semantic rejection of permission escapes rather than relying on unsupported-syntax errors.

Completion gate: the task list loads and saves safely, reports failures, and preserves existing data on failed writes. Async operations cannot bypass effect checking. Network/decode/cancellation failures have specified outcomes.

What the human checks:

- Run persistence and failure demonstrations, including cancellation and failed writes.
- Review async effect-escape results, interpreter/JavaScript agreement, and the decisions log; sign off.

## Milestone 5 — Prove all platform paths

Before platform implementation, inventory human prerequisites for the selected target OSes and packaging route. Record each dependency, responsible human, and readiness evidence in the milestone checklist. Mark unmet prerequisites `blocked-on-human` up front:

- SDK/toolchain installation and license acceptance, including required desktop build tools and mobile SDKs.
- Access to required target hardware/build hosts, including a macOS/Xcode host if Apple targets are selected.
- Developer accounts, signing certificates/keys, provisioning profiles, and registration when required by the selected packaging or distribution route. Humans supply credentials through appropriate secure tooling, never repository files.
- Emulator installation/configuration or physical-device setup, including debugging authorization and device permissions.
- Human execution/review of target-specific installation and lifecycle checks where the implementation session cannot access the environment.

Record prerequisites that are not required for a selected route as not applicable with a reason. Proceed with ready targets while blocked targets retain their status; blocked checks do not count as passed gates.

Execution order: web and CLI first, then desktop, then mobile. Resolve the prerequisite inventory before reaching the relevant target rather than discovering setup needs during implementation.

Keep one project with shared Ubi domain/application modules and small platform hosts:

| Target | Initial delivery | Required checks |
| --- | --- | --- |
| Web | Browser interface importing generated modules | Keyboard interaction, accessible controls, persistent storage, failure states |
| CLI | Node entrypoint | Commands, exit codes, persistent storage, readable errors |
| Desktop | Packaged web interface through a desktop shell | Launch, storage paths, window lifecycle, installation |
| Mobile | Packaged web interface through a mobile shell | Touch input, suspend/resume, persistence, device launch |

Select desktop/mobile shells after a small integration spike; likely candidates are Tauri and Capacitor. Record supported OS/runtime versions and unavailable platform checks explicitly.

Completion gate: each host runs the same Ubi task rules and persistence interface. At least one actual desktop package and one mobile emulator/device run are demonstrated. A browser preview alone does not count as mobile validation.

What the human checks:

- Confirm prerequisite readiness and run web/CLI, desktop, and mobile demonstrations in that order.
- Review packaging, accessibility, persistence/lifecycle evidence, remaining blockers, and the decisions log; sign off only when the existing completion gate is met.

## Milestone 6 — Make agent and human workflows reliable

- Build a canonical formatter from the syntax representation; preserve comments and require idempotence.
- Generate compact module interface summaries from compiler information, including exported types and effects.
- Add basic editor diagnostics, navigation, and hover through an LSP using compiler APIs.
- Expose versioned syntax output and a small structured-edit interface. Anchor edits to source revisions, preserve comments, and reject stale edits.
- Support ordinary executable Ubi tests, then add useful property testing where it finds defects.
- Establish a fixed set of agent tasks: implement a feature, repair a type error, extend an enum, decode data, and add a permitted host operation.
- Measure task correctness, compile/fix iterations, token use, and human review effort. Hidden behavioral checks must catch compilable but incorrect answers.

Completion gate: an agent using the compact spec can complete the benchmark tasks reproducibly. Report measured results without assuming agent superiority from syntax familiarity.

What the human checks:

- Try formatting, editor feedback, and a structured edit; inspect benchmark convergence and independently graded correctness, including hidden-test outcomes.
- Review comparison with the early benchmark, approve the decisions log, and sign off on the completion gate.

## Beyond the first release

Prioritize from reference-application and agent-benchmark evidence:

1. Language ergonomics: traits/interfaces, richer generics, and effect polymorphism where needed.
2. Application framework: shared UI authoring, routing, accessibility, state, lifecycle, and device integrations with explicit capability requirements.
3. Distribution: project manifests, reproducible dependency resolution, lockfiles, version compatibility, package publishing, and migration tooling.
4. Browser playground: compile the Rust compiler core to WASM and expose its diagnostics and generated output.
5. WASM program backend: profile real workloads first; specify the JS/WASM boundary, memory model, and semantic conformance suite.
6. Native backend and possible ownership features: separate design proposals driven by measured requirements.
7. Stronger verification: evaluate contracts, properties, and refinements against concrete bugs and diagnostic quality.

The compiler running in WASM and Ubi programs compiling to WASM are separate deliverables.

## Execution rules

- Work in milestone order; keep each change small and runnable, and commit each completed meaningful change.
- Maintain positive, negative, and runtime conformance cases as features land. Add targeted tests for lowering and foreign-boundary failures.
- Keep the current JavaScript backend passing conformance checks before introducing another backend.
- Avoid fixed delivery dates until the first two milestones establish implementation pace.
- Track progress against completion gates; mark platform work blocked when required SDKs or devices are unavailable.
- Next task: specify and implement the check/build CLI contract, run the generated M1 modules in a browser as well as Node, and fuzz the parser/checker with an external watchdog.
