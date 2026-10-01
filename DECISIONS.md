# Ubi decisions and milestone evidence

Current work: first M2 shared-logic slice complete under user direction. Local reassignment and non-generic records are implemented; the full M2 task-list scope remains unfinished. Historical M1 hidden evaluation remains unavailable.

## Decisions

Current language work: M2 local reassignment and non-generic records, under the
user's direction to continue application development without historical gates.

User direction, 2026-10-01: set aside milestone gates for the current task and
implement Windows recognition of `.ubi` files. This direction does not assert
that outstanding milestone evaluations passed.

Windows registration evidence: installed for the current user on 2026-10-01.
Native Shell `AssocQueryString` lookups for `.ubi`/`open` resolve installed
VS Code and the friendly name `Ubi source file`; the icon, quoted open command,
text metadata, and Open With entry are registered under HKCU. Re-running the
installer succeeds. `-WhatIf` preview and whitespace checks pass. Compiler code
is unchanged.

| ID | Decision | Reason / cost | Revisit |
| --- | --- | --- | --- |
| D001 | Rust library with thin CLI, one package; readable JavaScript ES modules first. | Matches PLAN.md; library accepts supplied sources independently of filesystem/host access. Avoid premature crate/backend splits. | Milestone 1 evidence, then profiling. |
| D002 | Signed 32-bit checked ints; truncating integer division; binary64 float arithmetic with explicit NaN, infinity, and signed-zero behavior. Direct unary minimum-int literals ignore whitespace/comments; grouped positive out-of-range literals remain invalid. | Portable, exact integer range; generated JavaScript must check operations. Overflow is fatal, not wrapping or an implicit Result. Runtime arithmetic faults remain runtime faults even when operands are constant. | New numeric types require a spec change and both backend expectations. |
| D003 | Immutable nominal records, enums, and lists; `let mut` rebinds locals only. Plain copying for v0. | Simple alias semantics and reviewable updates. O(n) list updates and potentially quadratic repeated append are accepted costs; immutable children can share storage. Persistent data structures deferred. | Profile task-list workloads before changing representation. |
| D004 | `?` accepts Result only, with identical error types and an explicit named-function Result return annotation. No implicit From conversion. | Error changes remain visible through explicit match/mapping; smaller initial checker. Option propagation and arrow return annotations deferred. | Mandatory review in Milestone 2; retain unless recorded spec revision changes it. |
| D005 | Unicode scalar strings; indexing returns Option for strings/lists. Value equality, strict operand types, no coercion. Built-in length selects from an established string/List type; untyped empty lists remain errors. | Avoid JavaScript UTF-16/truthiness/reference-identity leaks. Scalar access requires explicit backend handling; no automatic normalization, grapheme indexing, or hidden type defaults. | Domain evidence, without silently changing existing semantics. |
| D006 | Explicit relative named imports, canonical case-sensitive source IDs, no cycles or top-level execution. | Deterministic graph and export resolution in Node/browser; smaller library surface. CLI must enforce root containment. | Package/lifecycle work after first shared-logic milestones. |
| D007 | Typed pure arrows snapshot captures; only named functions may gain effects. | Reassignment cannot change captured values; callbacks cannot hide authority. Function types in exported signatures, effect polymorphism, and effectful callbacks deferred. | Milestone 2 capture review, Milestone 3 escape tests. |
| D008 | Diagnostic schema v1 uses half-open UTF-8 byte spans and SHA-256 source revisions; edits validate all revisions atomically. Public `.ubi` fixtures, Markdown documents, and JSON manifests pin LF through `.gitattributes`. | Stable machine interface and stale-edit protection across Unicode/line endings. Git checkout conversion cannot silently invalidate fixture/review hashes. Messages can improve while codes/spans remain stable. | Schema changes require versioning and migration evidence. |
| D009 | Milestone 0 specifies core and shared-domain profiles; adapter/effect syntax, async, packaging, and compiler budgets need pre-implementation revisions. | First phase produces a reviewable behavior contract and examples, not speculative compiler/framework scaffolding. Unsupported operations never fall through to JavaScript. | Before each owning milestone. |
| D010 | Conformance expectations are independently authored/reviewed against SPEC.md; hidden cases remain evaluator-owned. | Prevent self-grading and agreement on wrong results. Public static checks cannot prove executable semantics. | Each behavior change and milestone gate. |
| D011 | No TypeSafe/Jev integration in language semantics or diagnostics. | Consulted typesafe-ai guidance; grammar, types, arithmetic, and conformance are exact rules kept in code. A probabilistic judgment is unnecessary here. | Only a separately scoped AI product feature requiring model judgments. |
| D012 | Lexer emits keywords as a token category distinct from identifiers. Source whitespace is the four ASCII characters space, tab, line feed, and carriage return; either CR or LF ends a line comment. | Matches `SPEC.md`'s reserved keyword/IDENT distinction and keeps tokenization independent of host Unicode whitespace tables and line-ending conventions. | Only through a specification revision with reviewed examples. |
| D013 | Initial compiler budgets: 1 MiB per source, 4 MiB total source bytes, 64 modules, 500,000 tokens, and 32 expression-AST levels including binary and postfix chains. Exceeding a budget reports `UBI0090`. Exceeding these limits is a resource error rather than a language error. | Bound memory and recursive AST traversal for the first compiler/fuzzing slice while keeping the accepted corpus in scope; limits are deterministic and independent of host. The smaller nesting cap avoids stack exhaustion during recursive AST destruction on the supported Windows host. | Re-measure with corpus and fuzzing before raising any limit. |
| D014 | M1 execution is limited to 1,000,000 steps and 32 active named-function frames per exported invocation; each evaluated expression, executed statement, and named-function entry consumes one step. Exceeding either limit faults with `UBI-R0005`. | Bounds recursive or otherwise runaway execution consistently in the interpreter and compiled JavaScript, independently of host watchdogs. A 256-frame implementation overflowed the supported Windows test stack, so the lower deterministic cap protects the host while allowing the reviewed recursive examples. | Re-measure with conformance and workload evidence before changing. |
| D015 | The CLI accepts a canonical root-relative entry ID, loads only its transitive imports, and rejects any source path whose canonical target leaves the canonical project root. `ubi build` writes each reachable module as a same-layout `.mjs` file under `.ubi-build` by default (or the selected output directory), without cleaning unrelated files; output symlinks cannot redirect writes outside the canonical output directory. Exit codes are 0 success, 1 compile errors, and 2 usage/input/I/O/artifact errors; `--json` applies to compiler results, while operational errors remain on stderr. | Keeps source loading inside the CLI, matches the library's virtual-root import rules, and produces modules directly executable by Node and browsers without overwriting package metadata. Separating operational failures from source diagnostics preserves accurate compiler spans and source revisions. | Revisit if cross-host path tests or field use reveal incompatibilities. |
| D016 | Register `.ubi` source files per user with an editor open command; prefer installed VS Code, fall back to Notepad, allow explicit executable selection. Preserve existing default choices. | Gives Windows a named file type, icon, and Open With integration without machine-wide changes or executing arbitrary source on opening. | Dedicated editor or installer work. |
| D017 | Versioned `ubi.json` declares project name, shared entry, and one or more web/mobile/desktop/cli targets. Omitted CLI entry loads the config; multi-target projects require explicit selection, and build outputs are separated by target. | Supports shared logic across platforms without forcing a project into one app kind or implying finished host packaging. Strict typed JSON parsing uses Serde rather than a custom parser; compiler library semantics remain unchanged. | Platform-specific entries and packaging options when their hosts exist. |
| D018 | Land M2 in runnable slices: local rebinding and non-generic nominal records first. Record host arguments use serialized JSON; returned records are frozen field objects. Runtime record depth is capped at 32. | Enables shared data logic while bounding recursive work and avoiding inspection of arbitrary live host objects/getters. Records keep plain-copy update semantics and source/module nominal identity. | Generic/collection slices and richer serialized host codecs. |
| D019 | `ubi run` builds fresh ESM and invokes an exported function with Node.js from PATH, defaulting to `main`. Optional function selection and JSON host arguments reuse existing export validation. | Provides direct execution without a second backend, shell interpolation, or introducing ambient host capabilities into Ubi. | Application hosts and richer CLI argument conventions. |

## Milestone 0 evidence

- Bootstrap: PLAN.md existed; SPEC.md and DECISIONS.md were absent. No compiler/package or example corpus existed.
- Specification: SPEC.md 0.1 defines grammar, profiles, bindings, values, functions, modules, matching, error propagation, evaluation order, host boundaries, fatal faults, unsupported features, diagnostics, and edits.
- Corpus: independently authored under examples/; 40 cases (19 accepted, 21 rejected), 43 source files, 64 exported invocations, and 22 required compile diagnostics. The task-list interface and shared-domain fixtures target Milestone 2. A different agent from the author reviewed the source/oracle semantics.
- Static artifact validation: passed JSON parsing/schema, unique IDs, typed-value structure, source/root/entrypoint/export references, exact SHA-256 revisions, strict UTF-8 without BOM, LF source line endings, and diagnostic range/scalar-boundary checks. Rejected graphs contain no invocations. Both corpus author and implementation session checked artifact integrity; neither used compiler output or claimed executable conformance. No interpreter/compiler exists in Milestone 0.
- Independent specification review: clarified length overload resolution, unary float operator wording, qualified enum constructors, and minimum-int whitespace/grouping rules. All findings addressed.
- Independent source/oracle review: no remaining blockers. Separate reviewer manually checked all accepted values/fault priorities, rejected codes/primary spans, required profiles, and task-list contract against SPEC.md; additionally verified source revisions and diagnostic snippets. Review covered numeric edges, short-circuit/evaluation order, Unicode, immutable alias behavior, captures, map/filter order, nested/guarded patterns, generic instantiation, same-error propagation, explicit error mapping, modules, and domain error priority.
- Hidden corpus: not provisioned or run by this session. A human/separate evaluator must own it outside implementation workspace and credentials before Milestone 1 conformance evaluation. No contents or credentials belong in this repository.

Reviewed artifact identities (SHA-256 of exact working-tree bytes; fixture-source identities are pinned by the manifest):

| Artifact | SHA-256 |
| --- | --- |
| SPEC.md | `15f03bbb54180d9aee6037fbaa0d30a75e209f4b245a21032e0d5f0913f5c2b6` |
| examples/README.md | `b98305438ee9eacf9afd6daea78d78b0fabc7460ed93d60d7b310c2300189cfe` |
| examples/conformance.json | `2484bf834fa35afd61eeb87115ec11c5d742ec345ba3f06e9ff3c907f1dda116` |

Final staging check normalized the manifest's CRLF separators to LF without changing parsed JSON; source bytes and expectations remained unchanged. Markdown/JSON line endings are pinned alongside Ubi sources so reviewed artifact identities survive checkout conversion.

Independent author and reviewer were separate agents from this implementation session. This evidence is static specification/oracle review, not human sign-off, compiler conformance, backend execution, fuzzing, or hidden evaluation. The commit containing these artifacts is the concrete revision for human gate review.

## Human sign-off

Status: accepted.

- Reviewer: user in this Codex task, who explicitly approved the gate.
- Date: 2026-09-29.
- Reviewed revision: `0dd23e85f93fffcd2657f3c24d944c1197fd24e3` (Milestone 0 specification, decisions, corpus, independent review, and static evidence).
- Decisions accepted: D003 plain-copy immutable collections and their recorded costs; D004 same-error-only `Result` propagation without implicit conversion.
- Gate accepted: independent review found no blockers; representative expected values and diagnostics are unambiguous; grammar and examples agree; no behavior relies on unspecified JavaScript semantics.
- Approval: advance to Milestone 1.

Milestone 1 implementation changes that revise behavior must update SPEC.md and decisions before implementation. Changes invalidating this Milestone 0 evidence require renewed sign-off.

## Milestone 1 evidence

Status: implementation ready for M1 human gate review. The hidden corpus and separate evaluator were not provisioned and have not been run.

- Package: one `ubi-lang` Rust package with `ubi` binary; zero third-party dependencies. Generated Cargo build output is ignored.
- Lexer: UTF-8 byte spans; ASCII identifiers/numbers and fixed keyword tokens; supported punctuation; strict escapes; exact whitespace; non-nested comments retained by span; zero-width EOF token. Lexer tests were authored and reviewed independently against SPEC.md.
- Review: an independent implementation review found no concrete lexer or span issues against SPEC.md. Source management now reports invalid UTF-8 separately with byte-accurate spans.
- Source management: preserves exact original bytes, rejects BOM/invalid UTF-8 at precise byte spans, computes SHA-256 revisions, sorts canonical source IDs, and enforces D013 byte/module budgets. The test-first run failed before `SourceError` existed; source tests now cover invalid encoding, exact hashes including multi-block input, duplicate IDs, ordering, and all three source budgets.
- Parser: recursive descent handles imports, functions, primitive signatures, blocks, statements, calls, `if`, literals, and Pratt operators. AST tokens and comments retain UTF-8 byte spans; comparison chains are rejected; unsupported later-profile forms receive `UBI0003`. Test-first run failed with the parser module absent. Tests include every public core program, two-module syntax, front-end corpus errors, operator spans/precedence, and D013 token/nesting limits.
- Resolver/checker: canonical relative import paths stay inside the virtual root; missing exports and import cycles use path/name spans; duplicate top-level/local names, lexical scopes, unknown names, arity, primitive operators, return paths, integer/float literal limits, private return inference, and recursive annotation requirements are checked. Diagnostics are deterministically sorted. Existing accepted M1 core and runtime fixtures typecheck; required name/type/import/literal errors match primary snippets.
- Typed IR: error-free checked modules lower with expression types, lexical locals, early returns, branch/block structure, and resolved `FunctionKey` call targets; source spans stay attached. Unsupported or inconsistent checked AST states fail with `UBI0090`. Tests cover arithmetic and minimum-int lowering, imported calls, and a branch whose then path returns early.
- Interpreter: evaluates parser AST directly and independently of typed IR, with lexical function/block scopes, named calls, early return, selected `if` branches, short-circuit logic, checked i32 arithmetic, binary64 operations, and host argument validation. D014 sets one million source-level steps and 32 active named-function frames per exported invocation; a 256-frame implementation overflowed the supported Windows test stack, so tests verify the 32-frame limit faults cleanly. Faults retain stable runtime codes and nonempty messages; internal invariants use a host error rather than the adapter-only `UBI-R0004`.
- JavaScript backend: emits deterministic readable ES modules over typed IR, with mangled private bindings, named exports/imports, percent-encoded relative module specifiers, UTF-8-safe string escaping, left-to-right expressions, return propagation through value blocks, and checked integer helpers. Export wrappers validate host arguments; internal cross-module calls share D014 budgets. IEEE binary64 remains native JavaScript `Number` operations.
- CLI/API: the library exposes supplied-source check/build results, sorted revisions, and import requests while keeping filesystem access in the CLI. CLI tests cover reachable-import loading, canonical-root and symlink containment, `.mjs` artifact mapping, preservation of unrelated output files, compiler/operational exit codes, JSON schema parsing and escaping, source-accurate name/type spans, and source limits enforced before file reads.
- Conformance: the Node 22.22.1 harness ran all 29 invocations in the nine accepted M1 manifest programs against recorded values/fault classes. The independent AST interpreter ran those same fixtures plus six additional runtime cases (D014 limits, escaped imports, reserved exports, and string checks); all 35 expected outcomes matched. Invalid programs are refused by the compiler and interpreter before execution.
- Browser: CLI-generated `main.mjs` returned `15` for `compute(7)` in Node 22.22.1 and in Brave; the browser DOM reported `data-state="pass"` and visible result `15`.
- Fuzzing: 10,000 deterministic Unicode/token mutations ran through parser/checker (and codegen for clean inputs). A separate PowerShell process watchdog imposed a 60-second timeout; the test completed in under one second with no panic/hang.
- Verification: `cargo test --all-targets` passes 67 tests; `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check`, and `git diff --check` pass. The hidden corpus and separate independent evaluator were not provisioned or run, so no result is claimed for them. Human review of this revision remains the Milestone 1 gate.

## Pre-Milestone 2 mini agent benchmark

Run date: 2026-10-01. Compiler baseline: `03d0a34a033279b637deed7c7125bc90210aacc0`.

- Contracts and preset limit: `benchmarks/m1/tasks.md`; four core-profile tasks, maximum three compile/fix iterations each. One iteration is a written attempt followed by `ubi check --json`.
- Separate solution agent wrote only task sources and `attempts.md`, without reading evaluator artifacts or grading behavior. Repair converged in two iterations after the required faulty attempt produced `UBI0020`; two-module imports, guarded division, and early returns each converged in one. All four final checks passed; no iteration-limit failures.
- Separate evaluator derived 45 input/expected-result cases from task contracts and SPEC.md before reading solutions. Both source AST interpreter and generated JavaScript passed all 45 expectations, including boundary/overflow faults, lazy branches, signed division, and Unicode strings. The explicit-return AST check passed. The evaluator corrected its initial integer comparator to treat positive and negative zero as the same Ubi int; the initial result and rationale remain in `benchmarks/m1/evaluation.md`.
- Reproduction: `powershell -NoProfile -File benchmarks/m1/evaluate.ps1`, or the regular Cargo test suite. Evaluation adds a test-only module; production compiler behavior is unchanged. Node 22.22.1 and Rust 1.97.1 were used.
- Final verification: `cargo test --all-targets` passes 70 tests; Clippy with warnings denied, formatting, and `git diff --check` pass.
- Scope: public independent benchmark evaluation, not hidden conformance, a broad agent comparison, or human gate approval. No evaluator-owned hidden corpus was provisioned. M1 gate and human review of these benchmark outcomes remain pending.

## Project config evidence

User-directed application tooling, 2026-10-01; no language milestone gate claimed.

- `ubi.json` is parsed with typed Serde JSON input and strict schema validation. The CLI selects a declared target, loads the existing source/import graph, and separates default artifact directories by target. Explicit-entry mode stays compatible. `examples/project` provides a working four-target project.
- A separate agent derived 11 grouped CLI tests from SPEC.md sections 10/13 before inspecting implementation. Checks cover all four targets, identical generated ESM, module layout, working-directory/root selection, output overrides and preserved files, malformed/unknown/duplicate config fields, version/type/name/path/target errors, the exact 64 KiB boundary, explicit-entry isolation, and compiler-error JSON/exit behavior. No implementation mismatches found.
- `cargo test --all-targets`: 81 tests pass. Symlink scenarios were explicitly skipped because Windows denied symlink creation (error 1314), including an unsandboxed retry; those new containment scenarios remain unverified on this host.
- Clippy with warnings denied, formatting, and whitespace checks pass. The example builds cleanly for `web` into `.ubi-build/web/src/main.mjs`. Targets still emit shared JavaScript modules; no web UI, desktop/mobile package, or CLI application host is claimed.

## First M2 shared-logic slice evidence

Run date: 2026-10-01. SPEC.md 0.3 specifies this partial language expansion.

- Parser, checker, typed IR, AST interpreter, and JavaScript backend support non-generic nominal records, field access, immutable updates, structural equality, record imports, and mutable local rebinding. Exported JavaScript record arguments use validated JSON text; returned records are frozen.
- A separate conformance agent derived ten grouped tests against the specification before inspecting implementation. Both execution paths pass cases covering nominal identity, aliases, scopes, diagnostics, early returns, evaluation/fault order, hostile host inputs, prototype-like fields, float/unit encoding, and the 32-level runtime record limit.
- `examples/project` now creates and completes a Task. Generated web modules passed a browser check for immutable updates, Unicode, frozen results, and rejection of live host objects without reading getters. The browser displayed `PASS: shared Ubi task records, immutable updates, and host validation.`
- `cargo test --all-targets`: 91 tests pass. Fuzz coverage includes valid record/rebinding seeds and 10,000 deterministic mutations, completing in under one second under a separate 60-second watchdog. Windows symlink scenarios retain the previously recorded host limitation.
- Clippy with warnings denied, formatting, and whitespace checks pass. Lists, enums, generics, matching, closures, and the full task-list application remain future work; no full M2 completion gate is claimed.

## CLI execution evidence

User-directed `ubi run`, 2026-10-01. The existing build graph and containment
checks produce fresh modules before Node invokes an exported function.

- Six independently authored CLI groups cover default/selected exports, primitive/unit/record arguments, strings and JSON results, special floats, config targets and imports, compile/runtime faults, invalid arguments/options/exports, missing Node, and shell-looking paths/names/input treated as data. An initial positive-zero expectation was corrected after reviewing the clarified output contract: positive zero prints JSON 0; negative zero uses its tag.
- The example project runs without function selection and prints `Hello from Ubi!`; selecting `createTask` with JSON arguments prints the expected task record.
- `cargo test --all-targets`: 97 tests pass. Clippy with warnings denied, formatting, and whitespace checks pass. Previously documented Windows symlink test limitations remain. Node.js on PATH is required; this command adds no host capabilities to Ubi source.
