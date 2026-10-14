# Typed Pure Function Values Implementation Plan

> **For agentic workers:** Use superpowers:subagent-driven-development with explicit file ownership and independently authored conformance tests. Continuous execution is authorized; record decisions rather than reopening historical gates.

**Goal:** Reusable typed pure callbacks, returned closures and function storage across Ubi modules.
**Architecture:** Extend source type parsing into existing interned structural function signatures. Reuse closure execution and shared budgets. Separate Ubi module exports from the public host ABI.
**Tech Stack:** Rust compiler/interpreter, JavaScript ES modules, existing test harness.
**Spec:** SPEC.md section 23, alongside existing runtime/effect contracts.

## Global Constraints

- Preserve exact type matching, nominal identity, snapshot captures and purity.
- Preserve 1,000,000 steps, 32 call frames and existing compiler resource bounds.
- Reject capabilities in function signatures, effectful/async values and function-containing equality.
- Reject host calls with function-containing declared input/output before body execution.
- Keep implementation and independent expected results separately owned.
- Commit complete meaningful changes; preserve unrelated data and secrets.

## Review Focus

- Generic callbacks and nested function types resolve without leaking type parameters.
- Function-containing empty containers/unused variants cannot bypass host checks.
- Captures remain usable after return, and caller budgets survive indirect/imported calls.
- Effectful/async aliases and capability-containing generic substitutions remain rejected.
- Public-type privacy follows parameters and returns of nested function signatures.

### Task 1: Independent conformance

**Files:** tests/function_values.rs and targeted existing tests in src/lib.rs.
- [ ] Derive positive, negative and host-boundary expectations independently from SPEC23.
- [ ] Cover function parameters/returns, nested generics, aggregates/imports, captures, evaluation order, equality, effects and runtime budgets.
- [ ] Observe failures before implementation; retain exact expected values/diagnostics.

### Task 2: Frontend and effect checking

**Files:** src/parser.rs, src/analyzer.rs, src/effects.rs; formatter only if required.
- [ ] Parse `fn(T, ...) -> R` using current recursive type/nesting machinery.
- [ ] Intern structural signatures and permit function storage/Ubi exports while preserving privacy and equality checks.
- [ ] Preserve generic inference/substitution and prohibit capability-containing function signatures.
- [ ] Make independent positive/negative conformance pass; resolve concrete failures.

### Task 3: Runtime host boundary and integration

**Files:** src/interpreter.rs, src/js_backend.rs; src/ir.rs only if required.
- [ ] Reject public calls whose declared signatures contain functions before body execution, recursively including empty aggregates.
- [ ] Preserve internal Ubi imports, function invocation/captures and shared budgets.
- [ ] Pass interpreter/JavaScript independent expectations, host checks and full suite.

### Task 4: Examples, review and evidence

**Files:** examples/function-values/**, README.md, PLAN.md, DECISIONS.md.
- [ ] Add focused reusable-callback example and document current language scope.
- [ ] Independently review final diff and fix validated findings.
- [ ] Run formatting, denied-warning Clippy, full suite and relevant fuzz/watchdog checks.
- [ ] Commit changes and record actual evidence; leave unexecuted release requirements explicit.
