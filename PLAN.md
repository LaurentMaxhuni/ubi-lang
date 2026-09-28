# Ubi implementation plan

Status: proposed implementation roadmap. No compiler implemented yet.

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

## Milestone 0 — Specify observable behavior

- Write a compact `SPEC.md` covering grammar, bindings, values, functions, modules, matching, errors, and evaluation order.
- Define `int` width and overflow behavior, float behavior, division, equality, string indexing, and out-of-bounds collection access. Prefer signed 32-bit integers initially; require consistent behavior across backends.
- Define immutable record update semantics and distinguish reassignment from mutation.
- Define foreign-boundary validation, fatal faults, and unsupported operations.
- Create small valid/invalid examples, including the task-list module's intended interface.
- Specify diagnostic schema: version, code, severity, message, primary span, related spans, optional edits, and edit applicability. Use explicit offset units and source revision identity for edits.

Completion gate: representative programs have unambiguous expected values or diagnostics. Grammar and examples agree. No implementation depends on unspecified JavaScript behavior.

## Milestone 1 — Compile one complete program

Build a vertical slice before expanding the language:

1. Create one Rust package with library and CLI modules; split crates only when needed.
2. Add source management, lexer, and spans.
3. Add a recursive-descent parser with Pratt expression parsing; retain comments for later formatting.
4. Resolve names and check concrete function signatures and expressions.
5. Lower checked code to a small typed representation.
6. Emit readable JavaScript ES modules.
7. Implement `ubi check` and `ubi build`, including JSON diagnostics.

Scope: literals, immutable bindings, arithmetic, named functions, calls, `if`, and two-file imports. Keep generated output deterministic for identical inputs.

Completion gate: the same generated module computes the same result in Node and a browser. Unknown names and type mismatches fail with accurate spans and nonzero CLI exit status.

## Milestone 2 — Make the language useful for shared logic

- Add nominal records, field access, record updates, lists, and local reassignment.
- Add data enums, pattern bindings, and exhaustive matching, including nested patterns. Guards must not incorrectly establish coverage.
- Add basic generic types/functions, then `Option`, `Result`, and error propagation.
- Add the minimum iteration and collection operations needed by the task list.
- Implement the task-list domain entirely in Ubi with no host access.
- Check argument evaluation order and ensure lowering never duplicates side effects.

Completion gate: create, complete, validate, and filter tasks in both hosts using identical generated logic. Missing enum cases, invalid fields, mismatched errors, and forbidden mutation produce compile errors.

## Milestone 3 — Establish host access and effects

- Define a small typed adapter format for JavaScript imports. Keep adapter implementation visibly outside ordinary Ubi code.
- Introduce opaque capability values that ordinary Ubi code cannot construct.
- Add finite effect sets to named function signatures and validate their propagation through the call graph, including recursion.
- Start with storage and console capabilities; add networking when async support exists.
- Restrict effectful calls to named functions initially. Defer arbitrary effectful callbacks and effect polymorphism.
- Validate foreign values; convert expected exceptions/failures into typed errors. Specify treatment of unexpected adapter faults.
- Test with fake capabilities for deterministic behavior and failure injection.

Completion gate: a program cannot access a host operation without the required capability or omit a propagated effect. Malformed foreign data becomes a decoding error. Documentation identifies trusted adapters as a security boundary.

Effect declarations describe possible operations; scoped capability values control available authority. This stage does not promise isolation from malicious JavaScript dependencies.

## Milestone 4 — Add asynchronous application behavior

- Specify task creation, execution timing, awaiting, cancellation, and unhandled failure before implementing `async`/`await`.
- Ensure effects remain tracked when asynchronous work is created, passed, and awaited; deferring execution must not hide permissions.
- Add asynchronous storage and HTTP adapters returning explicit `Result` values.
- Decode JSON using explicit typed decoders initially. Do not treat a generic type parameter as validation.
- Define an explicit entrypoint with host-provided capabilities and asynchronous completion.

Completion gate: the task list loads and saves safely, reports failures, and preserves existing data on failed writes. Async operations cannot bypass effect checking. Network/decode/cancellation failures have specified outcomes.

## Milestone 5 — Prove all platform paths

Keep one project with shared Ubi domain/application modules and small platform hosts:

| Target | Initial delivery | Required checks |
| --- | --- | --- |
| Web | Browser interface importing generated modules | Keyboard interaction, accessible controls, persistent storage, failure states |
| CLI | Node entrypoint | Commands, exit codes, persistent storage, readable errors |
| Desktop | Packaged web interface through a desktop shell | Launch, storage paths, window lifecycle, installation |
| Mobile | Packaged web interface through a mobile shell | Touch input, suspend/resume, persistence, device launch |

Select desktop/mobile shells after a small integration spike; likely candidates are Tauri and Capacitor. Record supported OS/runtime versions and unavailable platform checks explicitly.

Completion gate: each host runs the same Ubi task rules and persistence interface. At least one actual desktop package and one mobile emulator/device run are demonstrated. A browser preview alone does not count as mobile validation.

## Milestone 6 — Make agent and human workflows reliable

- Build a canonical formatter from the syntax representation; preserve comments and require idempotence.
- Generate compact module interface summaries from compiler information, including exported types and effects.
- Add basic editor diagnostics, navigation, and hover through an LSP using compiler APIs.
- Expose versioned syntax output and a small structured-edit interface. Anchor edits to source revisions, preserve comments, and reject stale edits.
- Support ordinary executable Ubi tests, then add useful property testing where it finds defects.
- Establish a fixed set of agent tasks: implement a feature, repair a type error, extend an enum, decode data, and add a permitted host operation.
- Measure task correctness, compile/fix iterations, token use, and human review effort. Hidden behavioral checks must catch compilable but incorrect answers.

Completion gate: an agent using the compact spec can complete the benchmark tasks reproducibly. Report measured results without assuming agent superiority from syntax familiarity.

## Beyond the first release

Prioritize from reference-application and agent-benchmark evidence:

1. Language ergonomics: closures, traits/interfaces, richer generics, arrow syntax, and effect polymorphism where needed.
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
- Next implementation task: Milestone 0's compact specification and example corpus. This plan does not start compiler implementation.
