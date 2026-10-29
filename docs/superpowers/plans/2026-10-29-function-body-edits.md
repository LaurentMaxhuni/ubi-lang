# Revision-Checked Function Body Edits Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Allow agents to preview and safely apply one checked function-body replacement.

**Architecture:** Pure library preparation locates the parser body span, checks revisions, rejects injected declarations and validates a copied candidate graph. CLI reuses the current loader, diagnostics and staged source-replacement transaction, adding a graph recheck before installation.

**Tech Stack:** Rust, existing serde/serde_json and standard filesystem APIs.

**Spec:** SPEC.md section 10, Revision-checked function-body edits.

## Global Constraints

- No new dependency, provider, language behavior or automatic execution.
- Preserve every byte outside the old body. Preview and rejected edits never save.
- Request/source limit 1,048,576 bytes; existing project and compiler budgets remain.
- Preserve unrelated deleted `ubi-ui/docs/public/Inter.ttf` unstaged.

## Review Focus

- Stale exact-byte revisions, including CRLF and non-ASCII source, reject.
- Body injection, string/comment braces, unknown fields and malformed requests reject safely.
- Candidate errors use candidate source revisions; type-error repair is supported.
- Dependencies, target identity and read-only files are rechecked before saving.
- No-op/diff behavior, final newlines and failed-write cleanup preserve user bytes.

### Task 1: Pure prepared edits and preview

**Files:** Create `src/edit.rs`, `tests/edit.rs`; modify `src/lib.rs`.

**Interfaces:** `FunctionBodyEdit` strict Deserialize request; `Compiler::prepare_function_body_edit(&FunctionBodyEdit) -> Result<PreparedBodyEdit, EditError>`. Prepared getters return original/updated source; JSON fields and error variants follow SPEC.

- [x] Independently author API/CLI contracts from SPEC and confirm initial red.
- [x] Implement parser-bounded replacement, compiler candidate checks and unified diff.
- [x] Pass pure API tests for preservation, invalid bodies, revisions and diagnostics.

### Task 2: Preview/apply CLI and regression verification

**Files:** Modify `src/cli.rs`, `README.md`, `PLAN.md`, `DECISIONS.md`, and plan evidence. Tests remain independently owned in `tests/edit.rs`.

- [x] Add `edit`, required `--request`, explicit `--apply`, strict bounded request loading and JSON/human output.
- [x] Reuse `replace_sources`, adding a pre-install graph/path guard; preserve formatter behavior.
- [x] Run independent edit contracts and existing formatter/CLI/API regressions; independently review.
- [x] Run full default suite, format, Clippy and native-feature compilation; demonstrate preview/apply on an isolated fixture.
- [x] Record limits/evidence and commit the completed slice.

## Execution Ledger

- User approved the function/body/revision/preview/check design with “Go.” Implementation details chosen locally; no repeated approval required.
- Refreshed local code-only Graphify index and verified `Compiler`, `SourceSet`, parser spans, CLI loader and formatter transaction in owning source.
- Independent parser-budget regression found generic invalid-request reporting; route resource failures through candidate diagnostics. The CLI regression additionally exposed Windows/MSVC main-stack exhaustion before the parser depth guard. Add `build.rs` for an 8 MiB `ubi` stack reserve and independent `tests/cli_stack.rs`; retain depth limits and UI thread behavior.
- Independent final focused verification: 16 edit contracts and one existing-check stack regression pass. Four deterministic pre-install guard cases execute successfully; symlink fixtures cannot execute on this Windows host (privilege 1314).
- Independent review found no remaining correctness issue. Red logs are retained under ignored `.ubi-build/edit-red.log`, `edit-parser-budget-red.log`, and `cli-stack-red.log`; the initial 274-test full run passed before the two follow-up regressions.
- Isolated demo `.ubi-build/edit-demo-f4435587bcb045c5bfe0f3ca056fc124`: preview preserves source bytes, apply changes only `{ 1 }` to `{ 42 }`, revision changes, no build artifacts appear. Final Clippy/format/native-feature compilation pass.
- Final full suite passes all 276 tests, including existing web/native parity (204.37 seconds for that test). Full log retained in `.ubi-build/edit-final-tests.log`; no hidden evaluator, native OS/GPU rerun or overall milestone acceptance is claimed.
