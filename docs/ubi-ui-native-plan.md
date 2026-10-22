# Ubi UI Native Implementation Plan

**Goal:** Run a real Ubi application in an original Rust desktop UI runtime.

**Spec:** [native design](ubi-ui-native-design.md), implementing the supplied
Phase 1 brief. Execute sequential verified milestones in this checkout.

## Constraints and review focus

- Preserve existing compiler, JavaScript and browser UI behavior.
- Keep native desktop dependencies opt-in through `native-ui`.
- Reuse checked exports and existing tagged-enum/flat-view encodings.
- Reject malformed/oversized input; never commit partial model/tree updates.
- Test key reuse/removal, stale pointer capture, non-ASCII editing, clipped hit
  targets, zero-sized surfaces and GPU readback rather than presence-only checks.

## Milestones

- [ ] Promote the bounded interpreter through `Compiler::runtime()` and
  `Runtime::invoke(module, export, &[serde_json::Value])`; test typed round trips,
  private/invalid calls, faults and resource limits; commit.
- [ ] Establish six Rust crates. Validate `View`, reconcile stable keys, expose
  lifecycle/accessibility state and semantic themes. Test rejection and no-op
  behavior; commit.
- [ ] Implement retained Taffy layout and centralized input. Test grid/flex,
  constraints, resizing, scrolling, clipping, focus and controlled events; commit.
- [ ] Implement cached font measurement and batched GPU drawing with offscreen
  readback. Verify rendered pixels and text; commit.
- [ ] Implement winit platform lifecycle and opt-in `ubi ui` integration,
  transactional Ubi app adapter, source package and native Ubi gallery. Test
  model/event/view behavior and launch actual executable; commit.
- [ ] Add benchmark, documentation portal, native captures and honest platform/
  component status. Run workspace tests, formatting, Clippy, docs build and
  benchmark. Record results and commit.
