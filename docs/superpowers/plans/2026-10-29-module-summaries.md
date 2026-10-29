# Checked Module Summaries Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let agents inspect checked public module interfaces without reading implementations.

**Architecture:** Add a filesystem-independent summary builder over `Analysis` and `SourceSet`, exposed through `Compiler::inspect()`. Reuse CLI loading and diagnostic reporting; serialize summaries with existing serde dependencies.

**Tech Stack:** Rust, serde, serde_json.

**Spec:** SPEC.md, section 10, Checked module summaries.

## Global Constraints

- Jev remains deferred; no new dependency or language semantics.
- Preserve unrelated `ubi-ui/docs/public/Inter.ttf` deletion.
- No artifacts or execution from inspect; compiler failures emit no partial summary.
- Follow existing loader restrictions and diagnostic/exit-code contracts.

## Review Focus

- Imported nominal types retain module identity, including same-name declarations.
- Checked signatures, tuples, callbacks and generic constraints remain accurate.
- Strings, detached comments and preceding code cannot become documentation.
- Broken imports and invalid private bodies suppress the whole summary.
- Recursive nominal types terminate; expanded type strings have bounded size.

### Task 1: Add checked library and CLI summaries

**Files:** Create `src/inspect.rs`, `tests/inspect.rs`; modify `src/lib.rs`, `src/cli.rs`, `README.md` and milestone evidence.

**Interfaces:** `Compiler::inspect() -> Result<ModuleInspection, Vec<Diagnostic>>`; public serializable summary types in `src/inspect.rs`, re-exported from `src/lib.rs`. JSON fields follow SPEC exactly.

- [x] Independent test author derives API/CLI contract tests from SPEC, including all review-focus classes.
- [x] Run `rtk cargo test --locked --test inspect`; confirm missing API/command fails before implementation.
- [x] Implement analysis-backed summary builder, bounded type rendering and lexer-backed documentation.
- [x] Add CLI dispatch, config selection and JSON serialization without writing files.
- [x] Pass independent tests, existing compiler/CLI tests and full default-feature suite; run format and Clippy.
- [x] Independently review implementation, fix confirmed defects, record evidence and commit completed slice.

## Execution ledger

- Design follows user's accepted compiler-derived summary proposal; implementation details above chosen locally.
- Graphify `graphify-out/graph.json` located `Compiler`, `Analysis`, parser `Module` and API callers; owning source verified directly.
- Ruling: retain existing explicit export return annotations (UBI0021); private helpers can still infer types. Summary support does not change language semantics.
- Independent contracts: 10 pass; initial missing-API failure retained in ignored `.ubi-build/inspect-red.log`. Independent implementation review found no unresolved correctness finding.
- Real gallery invocation summarizes 74 modules and 246 exports in schema-v1 JSON. Formatting and denied-warning Clippy pass.
- Initial sandbox CLI regression run hit artifact-replacement access errors; full suite rerun with filesystem access passes: 254 tests across 19 suites, 196.93 seconds.
- `native-ui` feature compilation passes (114 crates); native GPU/OS tests were not rerun. No hidden evaluator or milestone acceptance is claimed.
