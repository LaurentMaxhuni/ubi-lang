# Ubi decisions and milestone evidence

## Ubi UI direction, 2026-10-21

User requested an Ubi-first UI system like shadcn for web, mobile and desktop,
then popular-framework support later. D033 direction: source-owned Ubi components
and typed state/messages with a bounded flat view arena, rendered through a small
semantic DOM host shared by existing Electron/Capacitor packages. Generic Ubi
helpers stay inside Ubi; concrete data-only exported wrappers preserve SPEC23's
host boundary. This is a design proposal, not an implemented library or an
assertion of native-widget support. The written design records options, visual
tokens, component waves, accessibility, failure behavior and acceptance checks.
After the user delegated the host choice, retain the shared DOM hosts for the
first release. Electron/Capacitor are packaging/runtime details below the Ubi
component API. The written foundation plan awaits review; no UI implementation
is claimed yet.

## Equality constraint direction, 2026-10-21

User requested continued roadmap work. Reusable list membership/deduplication
needs equality on generic elements; current universal generics correctly reject
`T == T` without evidence. D032: add the single built-in `T: Equatable` constraint
to generic functions/records/enums (SPEC section 25). Reuse existing equality,
inference and erased runtime generics. Check every instantiation, including
contextual function values, phantom arguments and recursive nominal types.
Keep arithmetic, ordering and user-defined traits deferred. No model judgment
or additional dependency is needed for exact compiler rules.

Equality constraint implementation evidence, 2026-10-21:

- Added optional Equatable bounds on function, record and enum parameters.
  Universal equality eligibility follows all aggregate components and nominal
  type arguments. Checked substitutions cover calls, contextual function values,
  constructors and type applications; a final scan revalidates forward nominal
  instantiations after fields/variants are populated. Constraint errors retain
  `UBI0020`, separately from genuine `UBI0090` resource failures.
- A separate author derived 10 interpreter/Node conformance groups covering
  collection algorithms, primitives/NaN, recursive and phantom aggregates,
  transitive/contextual callbacks, imports, host guards, invalid constraints,
  effects/capabilities and canonical formatting. The initial test failed with
  `UBI0002` at the new colon syntax before implementation; evidence is retained
  in ignored `.ubi-build/constraints-red.log`.
- Independent audit reproduced the old unconstrained phantom-argument equality
  escape and a forward-declared function-containing type accepted through an
  empty constrained record. Separate regressions now reject both with `UBI0020`.
  Composite conformance also exposed generic JavaScript equality using identity;
  generic operands now use the existing structural comparator. Fixed results
  agree with the independent AST interpreter, including nested NaN behavior.
- The imported reusable library implements membership, deduplication and
  frequency counting using Equatable, closures and tuples. The example prints
  `constraints: pear, apple, plum; pear=3, apple=2, plum=1; pear=true` and formats
  idempotently; a separate reviewer verified those expectations.
- Final full suite: all 222 tests pass under a separate 55-second process
  watchdog, including 10,000 mutations, new constrained seeds and growing-type
  checks. Formatting, denied-warning Clippy and whitespace checks pass.
  Independent final review found no unresolved blocker. Full logs are ignored
  under `.ubi-build/constraints-full-tests*`.
- Windows loopback tests pass with network permission; symlink fixtures still
  lack privilege 1314. New changes have not run on hosted platforms or against
  evaluator-owned hidden conformance. Crate version remains 0.1.0; no release or
  historical milestone gate is asserted.

## Tuples and destructuring direction, 2026-10-21

User requested autonomous progress on unfinished work and critical bugs. The
next language roadmap slice is immutable structural tuples and destructuring
(SPEC section 24). D031: reuse interned structural types, existing match
patterns, immutable aggregate validation and both execution paths. Add tuple
literal/type syntax, irrefutable immutable let patterns and correlated match
coverage. Defer positional access, mutable destructuring and record patterns.
Keep conformance expectations independently owned. Platform release gates and
hidden evaluator evidence remain separate.

Tuple implementation evidence, 2026-10-21:

- Implemented tuple types/literals, singleton/grouped/unit syntax, immutable
  nested let destructuring and correlated tuple match coverage across parser,
  analyzer, effects, IR, independent AST interpreter and JavaScript backend.
  Generic inference/substitution, captures, privacy, function-containing types,
  exact serialized host validation and existing limits include tuples.
- A separate author derived 17 conformance groups with fixed interpreter/Node
  expectations, negative diagnostics, host rejection before execution, ordered
  evaluation/early exits, aggregate depth, callback budgets and formatting.
  The initial smoke test against clean `a055a48` failed at missing tuple syntax
  (`UBI0002`); `.ubi-build/tuple-red-head.log` retains the result.
- Review identified exponentially copied/traversed shared tuple graphs. Reuse
  immutable Rc storage and memoize type substitution/traversal and aggregate
  depth within each operation. Type-substitution cache keys retain depth, so
  reuse cannot bypass the 32-level expansion bound.
- Independent audit reproduced a pre-existing shared-list equality hang and
  the same defect for tuples: 27 levels exceeded 15 seconds without exhausting
  the runtime step budget. Both backends now memoize aggregate comparison pairs
  per equality operation. They still inspect values before caching, preserving
  NaN inequality and nominal/ordered/map equality semantics. Separate tests
  compare shared and distinct graphs and retain a NaN control.
- Independent final review confirmed the cache limits, immutable bindings,
  product coverage, effect checks and host contracts; no unresolved findings.
- All 212 tests pass under a separate 55-second process watchdog, including
  10,000 frontend mutations and expanding tuple substitutions. Denied-warning
  Clippy, formatting and whitespace checks pass. Full logs are ignored under
  `.ubi-build/tuples-full-tests*`; the runnable cross-module tuple example prints
  `tuples: quotient=3, remainder=2, swapped=right, count=3` and formats idempotently.
- The sandbox denied loopback binding in two baseline dev tests; the complete
  suite passes with local network permission. Windows symlink fixtures still
  lack privilege 1314. New changes have not run on hosted platforms or against
  evaluator-owned hidden conformance; release requirements remain unchanged.

## Base language direction, 2026-10-14

User requested continued base-language development following the proposed
function-values/tuples/generic-constraints/library/tooling roadmap. First slice:
typed pure function values, including reusable callbacks, returned closures and
storage in existing aggregates. SPEC section 23 is authoritative.

D030: reuse the existing structural function-type representation, closure
execution and immutable captures. Add `fn(T) -> R` source types; keep async/effects
outside function values. Permit Ubi exports/imports while rejecting complete
function-containing signatures at the public host boundary before execution.
This enables library composition without trusting serialized function values or
introducing effect polymorphism. Independent tests cover both execution paths,
negative type/effect cases, host boundaries and resource budgets.

- Implemented source function types, exact structural checking, aggregate
  storage and Ubi module exports/imports using existing closure representations.
- Independently authored 15 conformance groups cover fixed interpreter/JavaScript
  results, captures, call order, generic inference, aggregates, privacy, public
  host rejection (including empty/phantom types), purity and runtime budgets.
  The first test failed at missing syntax before implementation.
- Independent review found phantom generic record/enum function arguments could
  still be compared. Separate negative tests reproduced the gap; a recursive
  function-type guard fixes it. Reviewer confirmed no remaining findings.
- Final verification: all 195 tests pass, formatting and denied-warning Clippy
  pass, and 10,000 parser/checker mutations plus expanding-instantiation checks
  pass under a separate 55-second watchdog. The cross-module example returns 28
  and passes canonical formatting. Full logs: ignored `.ubi-build/function-values-*`.
- The first full Windows run encountered loopback socket abort 10053 in three
  dev tests; isolated 6 and the final full 195 rerun pass without dev changes.
  Windows symlink fixtures still lack privilege 1314; existing Linux evidence
  remains separate. New language changes have not run on hosted CI or against
  evaluator-owned hidden conformance. Version remains 0.1.0.

## Hosted CI follow-up, 2026-10-14

- User approved publication; all 14 pending commits were pushed to `main`.
- First hosted run exposed an Android SDK command absent from `PATH`.
  Commit `2b0382e` uses the runner SDK's explicit `cmdline-tools/latest/bin`
  path. The subsequent Android debug APK build passes.
- macOS desktop source/package launch and unsigned iOS simulator builds pass.
  iOS UI, persistence/lifecycle, physical devices and signing are not established
  by compilation. Hidden conformance remains evaluator-owned and unavailable.
- The second run exposed intermittent HTTP 408 for a valid dev status request
  on macOS/Node24. Accepted sockets can inherit the listener's nonblocking
  mode; explicitly restore blocking I/O before applying the specified read/write
  timeouts. This preserves the existing 200ms/500ms bounds and HTTP behavior.
  An independent regression reproduced 408 before the fix; delayed-header and
  idle-timeout checks both pass afterward. All 180 local tests, formatting and
  denied-warning Clippy pass. Independent final review found no actionable issues.
- [Final hosted run](https://github.com/LaurentMaxhuni/ubi-lang/actions/runs/38598365263)
  passes all 11 jobs for `a6328bc`: 180 tests, formatting, denied-warning Clippy
  and portable packaging on Windows/Linux/macOS with Node22/24; native desktop
  source/package launches on all three OSes; Android APK and unsigned iOS build.
  [Hosted evidence](docs/hosted-ci-verification.md) records scope and remaining checks.

## V1 direction, 2026-10-09

User authorized autonomous work toward web, app/mobile, CLI, and desktop v1.
Ruling: interpret versions as platform targets, with explicit runtime floors;
arbitrary historical runtime compatibility is neither testable nor useful.
D027: immediate-await named async calls, sequential execution, and explicit
storage/HTTP capabilities preserve effect checking without first-class task or
effect-polymorphism complexity. Cancellation belongs to host-supplied signals.
D028: share compiled Ubi domain with thin web/Node/Electron/Capacitor hosts;
provide runnable packaging rather than target labels alone. Native toolchains,
hardware, signing, and actual package checks determine release readiness.
D029: project creation, packaging, and a token-preserving canonical formatter
complete the initial developer loop. Preserve existing files and failed-write
data. Independent conformance authors/reviewers evaluate implementation.
User's "Do whatever to get to v1" authorizes routine design/implementation
choices and continuous execution; no intermediate design approval is required.

## V1 integration evidence, 2026-10-09

- Checked named async functions and immediate await cover parser, checker,
  effects, IR, independent AST interpretation and JavaScript. Async storage/HTTP
  use scoped capabilities and cancellable host factories. Eight independent
  groups cover order, propagation, generic/import/alias escapes, delayed promises,
  malformed responses, unexpected failures, cancellation, budgets and concurrency.
- Independent tests/review found Windows interpreter stack overflow on legal
  recursive async source. Smaller control-expression dispatch frames preserve
  the specified 32-function-frame fault instead of host stack overflow; regressions pass.
- Portable task app shares Ubi rules across web, CLI, Electron and Capacitor.
  Concrete host decoding rejects corrupt state; failed writes preserve data.
  Project init, packaging, canonical formatting and version/async runner commands
  complete the initial developer loop.
- Review found valid source filenames requiring URL encoding in package imports,
  initial readiness controls, and generated artifact hardlink truncation. Fixes
  were reviewed; independent red-to-green build/package hardlink tests confirm
  unrelated linked bytes survive output replacement.
- Final local compiler evidence: 178 tests pass; `cargo fmt --all -- --check`,
  `cargo clippy --all-targets -- -D warnings`, and whitespace checks pass.
  Mutation/expanding-instantiation suite passes under a separate 55-second watchdog.
  Windows symlink tests cannot create fixtures (privilege error 1314), so remain
  unverified despite the overall green suite.
- Web UI and static bundle checked through browser interactions; CLI persisted
  task check passes. Actual Windows packaged executable passes hidden-window
  UI/module/persistence/sandbox smoke. Android debug APK builds/installs/launches;
  completion survives background/resume and process stop/relaunch on API37 emulator.
- [Platform evidence](docs/PLATFORMS.md) distinguishes executed checks from
  configured CI. Existing Ubuntu24.04 WSL2/WSLg now verifies Linux CLI persistence
  and the actual packaged desktop executable with sandbox enabled. Linux compiler
  verification exposed a dev-server socket-response failure; the minimal refusal
  fix passes independent dev/full178 checks and100 oversized probes. File/directory
  symlink prerequisites and containment tests actually execute on Linux.
  A separate full Linux run with Node24.21.0 also passes all178 checks.
  At that stage macOS/iOS, hosted CI, physical devices, release signing and hidden
  conformance remained unverified; publication was awaiting explicit approval.
  The 2026-10-14 follow-up above records publication and hosted results. Cargo version
  remains 0.1.0; no 1.0 release, human milestone sign-off, or store publication is claimed.
- Independent final CI review found Ubuntu24 Chromium sandbox prerequisites.
  Use Ubuntu22.04 for desktop CI without disabling the sandbox or changing system
  security policy; migrate before runner retirement on2027-04-17. Capacitor8 iOS
  SPM project command and exact dependency versions were reviewed, not executed.
- Linux reproduced an intermittent dev-server oversized-header failure: closing
  with unread input resets the socket after fragmented431 response writes, leaving
  a partial HTTP header. Refuse oversized headers by closing without a response,
  as allowed by the independent regression; retain existing memory/time limits
  instead of adding request-draining machinery. Clarified SPEC before implementation.
- Rust1.99 Clippy flags fixed-size SHA-256 chunk iterators. Use stdlib array chunks
  with the same algorithm; existing source-hash vectors verify unchanged digests.

Current work: user-authorized generics, Result/propagation, maps, then scoped host access. Completed foundations include local reassignment, records, lists/Option/match, loops, closures, pure utilities, and enums. Historical M1 hidden evaluation remains unavailable.

## Decisions

Current language work: user-directed M2 foundations and synchronous M3
console/storage capabilities, without reopening historical gates.

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
| D020 | Web development uses a Rust stdlib loopback HTTP server, in-memory compiled modules, metadata polling, and full-page reload. Only public web assets and current modules are served. | Reuses compiler and import containment without adding dependencies or requiring Node. Failed builds block module loading and recover after fixes. Reload discards browser state; file watching skips symlinks and config edits require restart. | Larger projects needing native watchers, persistent state, or hot updates. |

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
- Clippy with warnings denied, formatting, and whitespace checks pass. At this revision, lists, enums, generics, matching, closures, and the full task-list application remained future work; no full M2 completion gate was claimed.

## CLI execution evidence

User-directed `ubi run`, 2026-10-01. The existing build graph and containment
checks produce fresh modules before Node invokes an exported function.

- Six independently authored CLI groups cover default/selected exports, primitive/unit/record arguments, strings and JSON results, special floats, config targets and imports, compile/runtime faults, invalid arguments/options/exports, missing Node, and shell-looking paths/names/input treated as data. An initial positive-zero expectation was corrected after reviewing the clarified output contract: positive zero prints JSON 0; negative zero uses its tag.
- The example project runs without function selection and prints `Hello from Ubi!`; selecting `createTask` with JSON arguments prints the expected task record.
- `cargo test --all-targets`: 97 tests pass. Clippy with warnings denied, formatting, and whitespace checks pass. Previously documented Windows symlink test limitations remain. Node.js on PATH is required; this command adds no host capabilities to Ubi source.

## First web development evidence

User-directed `ubi dev` and clickable example, 2026-10-01.

- Config mode selects web automatically. The dev server compiles the reachable source graph into memory, serves loopback-only public assets/current modules, polls changes, and reports browser compilation errors before reloading on recovery. The existing CLI source loader is reused.
- Six independently authored integration groups cover startup without Node, port selection, imports, GET/HEAD/MIME/cache behavior, edits/creations/deletions, failed builds and recovery, invalid options, private paths, traversal, Host validation, methods, and published request/asset budgets. The external reload-script fixture was reviewed against the contract before accepting it alongside inline injection. Windows denied symlink creation (1314), so those containment scenarios remain unverified on this host.
- Browser testing created/completed tasks with Ubi functions, submitted by keyboard, displayed UBI0010 after a deliberate source error, and automatically recovered after restoring source. Desktop and 380px layouts were inspected; the narrow layout has no horizontal overflow. The demo stores its collection in browser JavaScript because Ubi lists remain future work; tasks reset on reload.
- `cargo test --all-targets`: 103 tests pass; Clippy with warnings denied, formatting, and whitespace checks pass. No new dependencies or Ubi language semantics were introduced.

## Programming foundations direction, 2026-10-03

User authorized the first four foundation groups: lists/Option/match, loops/ranges,
closures/collection helpers, strings/math/conversions. The task list is only a
historical example and is not the current implementation target. Maps, Result,
filesystem, classes, tuples, and host features remain later work.

D021: keep immutable lists and explicit local rebinding; lazy end-exclusive int
ranges avoid allocations; loops and callbacks share deterministic runtime limits.
Built-in List/Option inference is sufficient without generic user declarations.
Snapshots make closure capture stable. Use records for named data; defer richer
abstractions. ASCII trim and strict decimal parsing avoid host-specific coercion;
float string formatting and Unicode casing are deferred until portable contracts
are needed. Specify pure utilities in SPEC section 15 before implementation.

## Programming foundations evidence

Run date: 2026-10-03. User-directed scope, SPEC.md 0.4 section 15.

- Parser, checker, typed IR, independent source interpreter, and JavaScript
  backend implement immutable List/Option values, safe scalar/list indexing,
  exhaustive guarded/nested matching, for/while and lazy ranges, break/continue,
  typed snapshot closures, named function callbacks, map/filter/find/fold,
  string utilities, math, and strict explicit conversions.
- A separate conformance author derived twelve acceptance groups against the
  specification before inspecting production implementation. Fixed expectations
  cover aliases, evaluation/fault order, missing values, closure returns and
  nested/per-iteration captures, loop budgets, Unicode scalar strings, strict
  parsing, IEEE domains/signed zero, host validation/freeze, and 32/33 aggregate
  depth. Both interpreter and generated JavaScript pass those expectations.
- examples/foundations is a standalone language example; its documented main
  output and mathSample=12 pass through ubi run. The same generated logic passes
  a local browser verification page. No task-list changes were required.
- Frontend fuzzing includes three new valid foundation seeds and 10,000
  deterministic mutations. Public evaluation remains separate from the
  implementation; no evaluator-owned hidden corpus or full M2 gate is claimed.
- Follow-up review corrected lexical prelude-name shadowing in the interpreter
  and named-callee step accounting in JavaScript. Two additional separately
  authored regression groups cover local callback names/captures and exact
  source-expression work budgets. Fourteen foundation groups now pass. Checked
  fuzz programs must also build successfully, rather than tolerating internal
  lowering diagnostics. Independent production review found no remaining
  confirmed issue; unreachable match arms retain typechecking with a clarified
  contract. Oversized length reports the specified resource fault.
- A fifteenth conformance group verifies imported named callbacks with nested
  record/list/Option values, function-alias snapshot captures, frozen returns,
  and shared cross-module call-frame budgets.
- Final verification: 118 tests pass across all targets; Clippy with warnings
  denied, formatting, and whitespace checks pass. The stricter 10,000-mutation
  fuzz run completed under a separate 60-second process watchdog. Previously
  documented Windows symlink checks remain unavailable on this host.

## User-defined enum direction, 2026-10-04

User authorized the next language foundation slice: non-generic nominal enums.
D022: extend existing typed aggregates and matching with qualified constructors,
positional payloads, and nested patterns. Keep constructor syntax distinct from
function values; resolve local shadowing consistently. Pattern-matrix coverage
checks payload combinations with a finite work budget. Host values use exact
`tag`/`values` JSON envelopes and existing validation, freezing, and depth limits.
Generic declarations, Result, maps, and host access remain separate slices.
Specify this behavior in SPEC section 16 before implementation. Independently
sourced conformance must check fixed expectations on both backends.

## User-defined enum evidence

Run date: 2026-10-04. User-directed scope, SPEC.md 0.5 section 16.

- Parser, checker, typed IR, source interpreter, and JavaScript backend implement
  non-generic nominal enums, qualified constructors, multiple positional payloads,
  imports, nested/guarded patterns, structural equality, and recursive types.
- A separate author derived eleven conformance groups from the specification
  without inspecting production code. Fixed expectations pass on interpreter and
  generated JavaScript: payload correlations, nominal/public types, snapshot
  callbacks, shadowing, argument/fault order, exact host envelopes, prototype-like
  variant names, recursive freezing, mixed 32/33 aggregate depth, and the bounded
  coverage matrix. Invalid programs fail before lowering.
- Coverage uses whole pattern rows and an explicit stack with a 100,000-work-unit
  budget. Type equality eligibility likewise traverses nominal types iteratively.
  Interpreter enum values share immutable storage; splitting call evaluation out
  of the main expression frame preserves the specified 32-call runtime limit on
  the supported Windows stack. Existing recursive runtime tests pass unchanged.
- examples/enums runs through ubi run: main prints
  `enums: rectangle=12, total=22`; rectangle returns
  `{"tag":"Rectangle","values":[3,4]}`. Imports and enum exports use the same
  generated-module path as existing language features.
- Independent implementation review found no blocking production defect.
  Current-status documentation and historical deferred-syntax assertions were
  updated to agree with the enum slice.
- Verification: 129 tests across all targets pass; Clippy with warnings denied,
  formatting, and whitespace checks pass. Three enum seeds extend the existing
  10,000-mutation fuzz suite, which passes under a separate 60-second watchdog.
  Previously documented Windows symlink privilege checks remain unavailable.
  Evaluator-owned hidden tests remain unavailable; no full M2 gate is claimed.

## Further foundation direction, 2026-10-05

User authorized basic generics, then Result and same-error propagation, maps,
then host access/effects. Continue runnable slices without reopening historical
milestone gates. D023: check generic bodies universally, infer invariant type
arguments from inputs and expected output, and require generic return annotations.
Generic exports support Ubi imports; concrete wrappers define host-call types.
Retain independent source interpretation and separately authored conformance.
Specify each following slice before implementing it; no hidden-evaluation or
platform-readiness claims follow from public checks.

D024: retain D004's identical-error-only propagation. Reuse nominal enum
representation and coverage for built-in Result; host envelopes use tag/values
like user enums. Early Err propagates through all enclosing expression/control
contexts within its named function; fatal faults remain separate.

D025: Map<K,V> uses exact int/bool/string keys, insertion order, immutable plain
copying, and association equality. This avoids float NaN and generic equality
bounds. Host encoding is an entry array with duplicate-key rejection; prototype
names remain ordinary data. Named map helpers avoid overloading list callbacks.

D026: synchronous console/storage use explicit capability parameters and finite
named-function effect declarations. Reject effectful aliases and capability
captures/aggregates/generic escapes. Trusted callbacks return serialized typed
Result; malformed output becomes Decode, expected failure becomes Failure,
unexpected throws fault. Share capability registration through one generated
host module; CLI JSON never grants authority. Async and platform packaging remain
separate work.

## Generics, errors, maps, and host evidence

Run date: 2026-10-05. User-directed scope, SPEC.md 0.6 sections 17-20.

- Generic functions/records/enums check universally, infer invariant arguments,
  and retain concrete nominal identities. The interpreter substitutes source
  expression types independently of JavaScript's erased generic bodies.
  Expanding nominal/function instantiations have finite resource limits.
- Built-in Result shares enum encoding/coverage; same-error `?` propagates through
  nested expressions and loops while preserving fatal-fault behavior. Maps use
  immutable insertion-ordered entry arrays and association equality.
- Opaque Console/Storage tokens and named-function effects enforce scoped host
  access. Trusted synchronous callbacks return serialized Result; malformed
  responses become Decode, expected failures remain Failure, unexpected throws
  fault. Public exports enforce exact arity and validation; compiler-reserved
  internal imports preserve shared budgets without a public bypass argument.
- A separate author derived 21 conformance groups: generics 6, Result 4, maps 6,
  effects 5. Fixed expectations pass on interpreter and Node. Controls cover
  allowed generic callbacks and effectful recursion/imports; escapes cover
  aliases, captures, aggregates, generic substitutions, and composite callees.
- Independent review found a contextual generic capability escape through an
  if-selected function. Reference-level substitution checks close it; separately
  authored Console/Storage regressions reject it while pure int controls pass.
  Reviewer verified the fix and reported no remaining blockers.
- Focused examples print `generics: first=3`, `result: 15`, and
  `maps: apples, pears; total=8`. The explicit host example logs two messages and
  returns Ok("Explicit capabilities") using an in-memory store.
- Verification: 151 tests across all targets pass, with Clippy warnings denied,
  formatting, and whitespace checks. Six new fuzz seeds cover these features;
  10,000 mutations and growing-instantiation checks pass under a separate
  60-second process watchdog. Windows symlink privilege checks and evaluator-owned
  hidden tests remain unavailable. No full M2/M3 gate or packaging claim is made.
