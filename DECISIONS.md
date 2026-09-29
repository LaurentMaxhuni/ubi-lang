# Ubi decisions and milestone evidence

Current milestone: 1, Milestone 0 accepted by human. Compiler implementation in progress.

## Decisions

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

Status: in progress. Rust package, source/span management, lexer, parser, module resolver, Milestone 1 name/type checker, and typed IR lowering implemented. Interpreter and executable compiler pipeline remain pending.

- Package: one `ubi-lang` Rust package with `ubi` binary; zero third-party dependencies. Generated Cargo build output is ignored.
- Lexer: UTF-8 byte spans; ASCII identifiers/numbers and fixed keyword tokens; supported punctuation; strict escapes; exact whitespace; non-nested comments retained by span; zero-width EOF token. Lexer tests were authored and reviewed independently against SPEC.md.
- Review: an independent implementation review found no concrete lexer or span issues against SPEC.md. Source management now reports invalid UTF-8 separately with byte-accurate spans.
- Source management: preserves exact original bytes, rejects BOM/invalid UTF-8 at precise byte spans, computes SHA-256 revisions, sorts canonical source IDs, and enforces D013 byte/module budgets. The test-first run failed before `SourceError` existed; source tests now cover invalid encoding, exact hashes including multi-block input, duplicate IDs, ordering, and all three source budgets.
- Parser: recursive descent handles imports, functions, primitive signatures, blocks, statements, calls, `if`, literals, and Pratt operators. AST tokens and comments retain UTF-8 byte spans; comparison chains are rejected; unsupported later-profile forms receive `UBI0003`. Test-first run failed with the parser module absent. Tests include every public core program, two-module syntax, front-end corpus errors, operator spans/precedence, and D013 token/nesting limits.
- Resolver/checker: canonical relative import paths stay inside the virtual root; missing exports and import cycles use path/name spans; duplicate top-level/local names, lexical scopes, unknown names, arity, primitive operators, return paths, integer/float literal limits, private return inference, and recursive annotation requirements are checked. Diagnostics are deterministically sorted. Existing accepted M1 core and runtime fixtures typecheck; required name/type/import/literal errors match primary snippets.
- Typed IR: error-free checked modules lower with expression types, lexical locals, early returns, branch/block structure, and resolved `FunctionKey` call targets; source spans stay attached. Unsupported or inconsistent checked AST states fail with `UBI0090`. Tests cover arithmetic and minimum-int lowering, imported calls, and a branch whose then path returns early.
- Verification: `cargo test --all-targets` passes 35 tests; `cargo fmt --all -- --check` and `git diff --check` pass. The binary target currently reports dead-code warnings because the compiler library API is not wired into the CLI yet. Independent interpreter, JavaScript output, CLI/JSON diagnostics, fuzzing, and the Milestone 1 execution gate remain pending.
