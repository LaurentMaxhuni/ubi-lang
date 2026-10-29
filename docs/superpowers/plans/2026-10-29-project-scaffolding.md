# Project Scaffolding Command Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `ubi new <directory>` create a runnable project and show next steps.

**Architecture:** Alias the existing `Command::Init` path; reuse `package::init` unchanged. Print next commands after successful generation and document both spellings.

**Tech Stack:** Existing Rust CLI and embedded reference-app templates.

**Spec:** SPEC.md sections 10 and 22.

## Global Constraints

- No dependencies, new templates, implicit installs, execution, Git initialization or app launches.
- Preserve existing file/symlink preflight and unrelated deleted font.

## Review Focus

- New command produces the same complete four-target starter as init.
- Existing template files are never overwritten or partially regenerated.
- Missing/extra/unsupported arguments cannot create files.
- Paths with spaces stay data; next commands contain no interpolated shell fragments.
- Success instructions require running from the generated project directory.

### Task 1: Expose the existing scaffolder

**Files:** Modify `src/cli.rs`, `README.md`, `SPEC.md`, `DECISIONS.md`, `PLAN.md`; independently author `tests/scaffold.rs`.

- [x] Independently verify new-command behavior/no-clobber/invalid arguments and confirm missing-command red.
- [x] Reuse `Some("new") | Some("init") => Command::Init`; print directory and next commands only after success.
- [x] Run new contracts plus existing package/CLI/v1 regressions, formatting, Clippy and whitespace checks.
- [x] Record evidence and commit this completed command.

## Execution Ledger

- Inspection found the complete embedded starter in `src/package.rs::init`; no template changes needed. Direct lookup used for this small change in known files.
- Independent contracts: three pass; missing-command red retained in `.ubi-build/scaffold-red.log`. Final review found no unresolved finding.
- Scoped CLI/package/API regressions: 88 pass; formatting, denied-warning Clippy and whitespace checks pass. Full logs retained in `.ubi-build/scaffold-regressions.log` and `scaffold-clippy.log`.
- Existing Windows symlink privilege limits remain; no unrelated UI-matrix/native OS or hidden-evaluator rerun is claimed.
