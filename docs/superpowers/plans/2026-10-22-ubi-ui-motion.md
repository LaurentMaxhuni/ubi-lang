# Ubi UI Motion and First Components Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver shared motion, seven existing control upgrades, and four real animated Ubi components.

**Architecture:** Keep logical Ubi views authoritative. A pure, bounded presentation engine samples host-owned tracks; browser/native hosts own scheduling, geometry and rendering. Implement this plan before the linked application and expressive component plans.

**Tech Stack:** Existing Ubi compiler/runtime, Rust workspace, winit/wgpu/Taffy, framework-independent JavaScript/CSS, Node tests and existing Next documentation. No new animation dependency.

**Spec:** [Approved motion/component design](../specs/2026-10-22-ubi-ui-motion-design.md), approved by user on 2026-10-22 with "i approve."

## Global Constraints

- Build original, source-owned `.ubi` components for the existing browser and Rust hosts. React remains documentation infrastructure.
- Existing constructor signatures and imports stay available. Schema-1 views without new properties remain valid.
- Validate duration `0..2000`, delay `0..1000`, opacity `0..100`, scale `25..200`, and each translation coordinate `-100000..100000`.
- Permit at most 4096 live nodes and 4096 visual exit nodes per mount.
- Never retain password/input values in detached exit snapshots; editables disappear immediately.
- Rust/JavaScript fixture samples must agree within `0.00001`.
- Frames never call `update` or `view`. No permanent idle redraw, ambient effects off under reduced motion.
- Use `rtk` for supported commands. Commit each completed task; do not claim unavailable platform checks.

## Review Focus

1. A second update during animation must start at the visible value, including delayed tracks (Task 2).
2. A failed reducer/view/DOM commit must not cancel or advance the previous animation (Task 3).
3. Removed keys reused before exit completion must not retain duplicate IDs or editable values (Tasks 2–3).
4. Nested scale/translation and clipping must agree with pointer/focus geometry at non-unit DPI (Task 4).
5. Hidden windows and live preference changes must settle finite motion, release resources and stop background work (Tasks 3–4).

## File map and subsequent plans

- `ubi-ui/packages/ubi-ui/core.ubi`: public typed ABI.
- `ubi-ui/crates/ubi-ui-core/src/motion.rs` (new), `src/lib.rs`: validated motion types, tracks, lifecycle and snapshots.
- `ubi-ui/web/motion.mjs` (new): matching pure sampling, no DOM dependencies.
- `ubi-ui/web/renderer.mjs`, `renderer.css`: transactional DOM integration and scheduling.
- `ubi-ui/crates/ubi-ui-layout/src/presentation.rs` (new): presented scene geometry, shared by painting/input/accessibility.
- `ubi-ui/crates/ubi-ui-render/src/paint.rs`, `src/lib.rs`: transform/alpha-aware painting.
- `ubi-ui/crates/ubi-ui-platform/src/motion.rs` (new), `src/lib.rs`: preference and redraw scheduling.
- `ubi-ui/fixtures/motion.json` (new): language-neutral easing/lifecycle cases.
- `ubi-ui/tools/web_export.rs`: include new runtime modules in every standalone export.
- Subsequent plans: [application controls](2026-10-22-ubi-ui-application-components.md), then [expressive components](2026-10-22-ubi-ui-expressive-components.md).

Graph navigation uses existing `graphify-out/graph.json`. Verified source anchors:
`View::decode`, `Tree::reconcile`, `Input::hit`, `Renderer::prepare`, native
`Host::dispatch`/`refresh_scene`, browser `mount`/`patch`. The unqualified Graphify
`mount` lookup finds the older example host; use `ubi-ui/web/renderer.mjs` directly.
Refresh the graph after source changes before relying on new relationships.

## Task 1: Validate the shared motion ABI

**Files:** modify `ubi-ui/packages/ubi-ui/core.ubi`, `ubi-ui/crates/ubi-ui-core/src/lib.rs`, `ubi-ui/web/validate.mjs`; create `ubi-ui/crates/ubi-ui-core/src/motion.rs`, `ubi-ui/crates/ubi-ui-core/tests/motion.rs`; extend `ubi-ui/web/tests/validate.test.mjs` and `tests/native_components.rs`.

**Interfaces:** Ubi exports exactly `Easing`, `Transition`, `Motion` and six motion properties from the spec. Rust exports equivalent enums and `Motion { duration_ms: u32, delay_ms: u32, easing: Easing }`; normalized Node gains optional motion, enter/exit and target opacity/translation/scale. Opacity defaults to 1, translation to `[0,0]`, scale to 1 after normalization. A missing Motion stays distinguishable from explicit zero duration.

- [ ] Add matching `motion_bounds_and_shape` cases in Rust and Node: accept all boundary values, reject duration 2001, delay 1001, opacity 101, scale 24/201, translation 100001, fractional integers, duplicate props, unknown easing, extra record fields and malformed nested enum payloads. Assert old examples still decode unchanged.
- [ ] Run `rtk cargo test --locked -p ubi-ui-core --test motion` and `rtk proxy node --test ubi-ui/web/tests/validate.test.mjs`; confirm failures identify unsupported new ABI, rather than setup errors.
- [ ] Implement public types and both decoders using existing validation/error conventions. Keep all existing graph/codec budgets; do not allow arbitrary CSS or unknown properties.
- [ ] Run those commands again plus `rtk cargo test --locked --test native_components`; expect all assertions passing with the catalog still at 40.
- [ ] Commit as `feat(ui): add validated shared motion contract`.

## Task 2: Pure frame sampling and bounded lifecycle

**Files:** extend Rust `src/motion.rs` / `tests/motion.rs`; create `ubi-ui/web/motion.mjs`, `ubi-ui/web/tests/motion.test.mjs`, `ubi-ui/fixtures/motion.json`.

**Interfaces:** Rust `sample(from: f64, to: f64, start_ms: u64, now_ms: u64, motion: Motion) -> f64`; JS `sample(from, to, startMs, nowMs, motion) -> number`. `motion` in JS uses `{durationMs,delayMs,easing}`. Public-to-host normalization supplies the same enums.

Both modules export `MotionState`. Rust methods are `commit(&mut self, targets: &[MotionTarget], now_ms: u64, reduced: bool) -> Result<(), Error>`, `sample(&mut self, now_ms: u64, reduced: bool) -> MotionFrame` and `dispose(&mut self)`; JS uses corresponding plain data and throws on invalid targets. `MotionTarget` contains `key: String`, `kind: Kind`, `channels: BTreeMap<String,f64>`, resolved `Motion`, enter/exit `Transition`, and `snapshot: Option<VisualSnapshot>`. `MotionFrame` contains per-key sampled channels, visual-only exits, `active: bool` and `next_wake_ms: Option<u64>`. VisualSnapshot owns sanitized Node/style/geometry data and sanitized descendants; recursively omit editable subtrees, never storing raw model/view JSON. All engine state remains independent of DOM, GPU, compiler and layout crates.

Channels include opacity, translate-x/y, scale and internal control values; channels are host-owned, never arbitrary executable source properties. Sanitize snapshots before storing them; input/password/text-area/number-input nodes have no exit snapshots. Every node owns a bounded set of channels; cap at 16 per node. Reject nonfinite host targets before mutating state.

- [ ] Add shared fixtures and `samples_match_reference_curve`: assert exact endpoints, linear midpoint 0.5, EaseOut midpoint 0.875, EaseInOut midpoint 0.5, Spring formula from spec, delay boundaries and zero-duration immediate target. Store reference numbers in JSON; each runner compares to fixtures, not just to its own formula.
- [ ] Add `interruption_and_exit_lifetime`: retarget a 0→100 linear track at 50ms of 100ms and assert new start 50; a delayed retarget starts from the last presented value; same-key/same-kind resurrection cancels exit; different kind replaces it; 4097 exiting nodes retain only the newest 4096; editable snapshots retain no content/value. `dispose()` leaves no tracks/exits.
- [ ] Run `rtk cargo test --locked -p ubi-ui-core --test motion` and `rtk proxy node --test ubi-ui/web/tests/motion.test.mjs`; verify new sampling/lifecycle tests fail before implementation.
- [ ] Implement spec easing, endpoint clamping, interruption, bounded snapshots and deterministic sampling. Finite transitions jump to elapsed-time targets after suspension; reduced motion settles all tracks and releases exits immediately.
- [ ] Repeat both suites; expect all shared samples within `0.00001` and zero retained state after disposal. Commit as `feat(ui): sample bounded keyed motion across hosts`.

## Task 3: Browser integration, rollback and idle scheduling

**Files:** modify `ubi-ui/web/renderer.mjs`, `renderer.css`, `ubi-ui/tools/web_export.rs`, `ubi-ui/web/tests/browser.mjs`; create `ubi-ui/web/presentation.mjs` and `ubi-ui/web/tests/presentation.test.mjs`; update `tests/web_ui.rs` for runtime export coverage.

**Interfaces:** `mount(root, app, options)` preserves existing options/handle methods. Add `options.motion` equal to `'system' | 'full' | 'reduced'` (default system) and optional host clock `{now(), requestFrame(callback), cancelFrame(id)}`. It is a trusted embedding/test interface, not Ubi capability access. Add `handle.setMotion(mode)` with the same validation. Production clock uses monotonic time and requestAnimationFrame. Export `createPresentation(root, options)` returning `prepare(nextView, entries)`, `commit(prepared)`, `sample(nowMs)`, `setMotion(mode)` and `dispose()` from the new DOM module; prepared state is private until successful logical/DOM commit.

- [ ] Add `failed_patch_preserves_motion` and `exit_reuse_preserves_focus`: deliberately fail a DOM patch after preparing motion; assert model/view, active track, focus/selection and DOM ownership remain unchanged. Remove/reinsert a key and assert one live ID, no live exit bindings and no editable snapshots.
- [ ] Add fake-clock tests: 100 frame callbacks do not invoke app reducers/views again; reduced-motion change settles and cancels callbacks; hiding cancels ambient scheduling, resuming samples elapsed finite time; disposal removes media/visibility listeners and cancels the frame. Scope all CSS and media handling to each mount.
- [ ] Run `rtk proxy node --test ubi-ui/web/tests/presentation.test.mjs`; serve repo root with `rtk proxy python -m http.server 4318 --bind 127.0.0.1`, open `/ubi-ui/web/tests/browser.html` through the browser tool and confirm new regression failures before integration.
- [ ] Integrate prepared presentation commits after successful DOM/state commit; rollback cannot mutate previous tracks. Snapshot exits without live IDs/event listeners. Add transforms/opacity via scoped style variables; browser hit geometry follows the same transforms. Export all imported runtime modules by extending the existing explicit exporter allowlist.
- [ ] Repeat Node/browser checks; run `rtk cargo test --locked --test web_ui`. Verify exported preview pages load their new relative module imports. Commit as `feat(ui): animate browser views transactionally`.

## Task 4: Native presentation, transformed input and frame scheduling

**Files:** create `ubi-ui/crates/ubi-ui-layout/src/presentation.rs`, `ubi-ui/crates/ubi-ui-layout/tests/presentation.rs`, `ubi-ui/crates/ubi-ui-platform/src/motion.rs`; modify corresponding crate `src/lib.rs`, render `src/paint.rs`, input `src/lib.rs`; extend layout/input/GPU tests, native host tests and `ubi-ui/tools/capture.rs`.

**Interfaces:** layout exports `present(view: &View, scene: &Scene, frame: &MotionFrame) -> PresentedScene`, where `PresentedScene` contains live `Scene` geometry plus per-key effective opacity/scale and visual-only exit layers. Compose ancestor transforms around node centers. `Renderer::prepare` remains a static-compatible wrapper; add `prepare_presented` with the same arguments plus `&PresentedScene`. Painting consumes presented geometry and applies text/quad transforms consistently. Input/accessibility consume the live presented Scene only. Visual exits never enter it.

Platform `MotionPreference { System, Full, Reduced }` defaults to System. Preserve `run`/`Options`; add `run_with_motion(app: impl Application + 'static, options: Options, preference: MotionPreference) -> Result<(), Error>`. Host timestamps come from `Instant`; use MotionState active/deadline to request redraw/WaitUntil and return to Wait when settled. Add `capture_at(app: &impl Application, from: &View, path: &Path, width: u32, height: u32, dark: bool, now_ms: u64, preference: MotionPreference) -> Result<String, Error>`; seed from the previous View at time zero, commit current app.view at zero, and sample at now_ms. Preserve capture's existing create-new-file behavior. Platform preference adapter returns reduced on query failure. Windows queries `SPI_GETCLIENTAREAANIMATION` and rechecks on activation and before scheduling motion; unsupported OS adapters use reduced. Verify current official Windows API documentation before implementation, then use a target-only direct `windows-sys` dependency matching the already locked version; do not import an ambient shell setting.

- [ ] Add `nested_transform_hit_and_clip`: a translated/scaled child under a transformed clipped parent produces the same visible/click/focus bounds at DPI 1 and 2; old coordinates do not hit the moved child; exit layers do not appear in hit/accessibility output. Add GPU timestamp captures whose pixel bounds match presented geometry.
- [ ] Add `host_sleeps_after_motion` and `preference_change_settles`: finite motion requests frames until its deadline, then no pending redraw/timer; occluded/zero-size windows do not spin; preference change removes exits/ambient loops; failed application dispatch preserves tracks.
- [ ] Run `rtk cargo test --locked -p ubi-ui-layout -p ubi-ui-input -p ubi-ui-platform -p ubi-ui-render`; verify focused new assertions fail before integration. GPU unavailability is an explicit environment limitation, not a pass.
- [ ] Implement presented geometry, painter transforms/alpha and native host scheduling. Expand render primitive budgets for exits only within the existing bounded frame limit; release snapshots on close/device teardown. Keep application commit atomic.
- [ ] Repeat focused suites; run `rtk cargo run --locked --features native-ui -- ui examples/gallery.ubi --root ubi-ui --smoke`. Record Windows full-motion and reduced-motion evidence and idle frame count. Commit as `feat(ui): render and schedule native motion`.

## Task 5: Upgrade existing controls without inventing catalog entries

**Files:** modify `ubi-ui/packages/ubi-ui/components/{tabs,segmented-control,accordion,collapsible,switch,progress,skeleton,button}.ubi`, shared core types/decoders for disclosure grouping, both presentation modules, native painting/layout; extend `tests/native_components.rs`, input/layout tests and browser regressions.

**Interfaces:** public constructor signatures stay unchanged. Add `Prop.Disclosure(bool)` on containers; these containers always exist, with logical content represented only while open and visual snapshots supporting close. Measured disclosure height adjusts sibling geometry each frame. Tabs/segmented controls resolve the selected child's stable key using existing Group/Selected semantics. A missing Motion resolves the spec defaults; explicit zero disables all motion, including shimmer. Control channels live in host MotionState.

- [ ] Add `control_motion_defaults_and_opt_out`: assert 140ms EaseOut feedback, 220ms Spring selection/switch, 220ms EaseInOut disclosure, immediate semantic selected/value changes and static zero-duration output. Verify old imports/signatures and all 40 examples compile.
- [ ] Add `disclosure_reverse_resize_and_focus`: close while a child is focused, assert immediate focus cleanup and noninteractive visual exit; reopen halfway without a jump; resize during expansion updates target/siblings without replaying entry. Skeleton reduced mode is static and schedules no callbacks.
- [ ] Run focused Rust/Node/browser checks from Tasks 1–4; confirm new component assertions fail first.
- [ ] Implement control-part sampling and source disclosure markers; keep tab panel semantics active-only and the focus ring immediate. Do not add duplicate Motion/Role props in source helpers.
- [ ] Run focused checks plus `rtk cargo test --locked --test native_components --test web_ui`; expect catalog 40 and all old behavior preserved. Commit as `feat(ui): animate selection disclosure and feedback controls`.

## Task 6: Four new animated components and real examples

**Files:** create `ubi-ui/packages/ubi-ui/components/{number-ticker,text-reveal,animated-list,shimmer-button}.ubi` and matching `ubi-ui/examples/components/<slug>.ubi`; modify `core.ubi`, `components.ubi`, both decoders/renderers, `ubi-ui/catalog.json`, `ubi-ui/docs/app/component-notes.js`, `ubi-ui/docs/app/guide-content.jsx`, `ubi-ui/tools/verify_web.mjs`, native/web tests and capture tooling.

**Interfaces:** add `Effect { NumberTicker, TextReveal, AnimatedList, Shimmer }`, `Kind.EffectSurface`, `Prop.Effect(Effect)` and `Prop.Number(int)` (signed 32-bit; only NumberTicker). NumberTicker and TextReveal are leaves; AnimatedList accepts children. Shimmer is valid on Button; other Effect values require EffectSurface. Effect/Kind pair mismatches fail validation. Constructors:

```ubi
numberTicker<M>(key: string, value: int, props: List<Prop<M>>) -> View<M>
textReveal<M>(key: string, content: string, props: List<Prop<M>>) -> View<M>
animatedList<M>(key: string, props: List<Prop<M>>, children: List<View<M>>) -> View<M>
shimmerButton<M>(key: string, content: string, message: M, props: List<Prop<M>>) -> View<M>
```

Use signed decimal formatting for ticker, including negative values; clamp overshoot before integer display. Announce only logical values. TextReveal exposes one continuous accessible string; per-word visual runs are hidden from semantics. AnimatedList reorders keyed child positions without changing reading order. ShimmerButton retains standard Button busy/disabled behavior. All four use motion defaults/overrides from earlier tasks; no imperative frame calls in Ubi.

- [ ] Add `first_motion_components_roundtrip`: compile each real Ubi example, compare native/JS init/update/view, assert expected kinds/effects and four unique catalog entries. Replace hard-coded 40/41 assertions in `tests/native_components.rs`, `tests/web_ui.rs`, `ubi-ui/tools/verify_web.mjs` and browser catalog scripts with catalog-derived expected counts (catalog plus one gallery); assert exact catalog count 44 at this release checkpoint.
- [ ] Add `ticker_negative_interrupt_and_reveal_semantics`, `animated_list_reorder_keeps_focus`, `shimmer_busy_does_not_activate`: sample intermediate values, interrupt, toggle reduced motion, reorder a focused child, verify one semantic string/value and zero unexpected actions.
- [ ] Run matching focused checks and confirm failures before adding source/host behavior.
- [ ] Implement source modules, facade forwarders and host effects; add reducers showing update/replay/reorder/busy toggles. Document copyable source, named effects, motion toggles and honest native/browser support.
- [ ] Run `rtk cargo test --locked --test native_components --test web_ui`, browser component catalog and timestamp GPU captures; verify 44 components plus gallery. Commit as `feat(ui): add first four animated Ubi components`.

## Task 7: First release verification and evidence

**Files:** update `docs/ubi-ui-motion-verification.md` (new), relevant docs platform/theming copy and `PLAN.md`; use existing capture/preview outputs under `ubi-ui/docs/public`.

**Interfaces:** existing real-preview routes and source highlighting stay functional. Documentation distinguishes sampled GPU stills from motion interaction evidence and names hosts actually exercised.

- [ ] Run `rtk cargo test --locked --workspace --features native-ui`, `rtk proxy node --test ubi-ui/web/tests/*.test.mjs ubi-ui/docs/tests/*.test.mjs`, `rtk cargo fmt --all -- --check`, `rtk cargo clippy --workspace --all-targets --features native-ui -- -D warnings`, and `rtk proxy npm --prefix ubi-ui/docs run build`. Retain full failing logs when output is condensed.
- [ ] Serve repo as in Task 3; exercise browser regressions and `/ubi-ui/tools/browser_catalog.html`, live documentation previews, copy/theme/reset, 320/768/1440px widths, full/reduced modes and multiple independent mounts. Capture native start/mid/end frames plus real window interaction; verify no idle redraw after finite completion. Record performance measurements without claiming an unmeasured frame-rate guarantee.
- [ ] Review the combined diff against all five Review Focus conditions, hostile ABI handling, focus/security budgets and source ownership. Fix concrete failures and rerun only affected checks before repeating final required checks.
- [ ] Record exact commands, results, screenshots and remaining OS/device/screen-reader limitations. Mark completed tasks truthfully and commit as `docs(ui): record animated foundation verification`.

## Handoff

Representative assertions belonging in the named test steps above:

```rust
// Task 1: invalid ABI input.
assert!(View::decode(invalid).is_err());
// Task 2: shared fixture sampling, not self-derived expectations.
assert!((actual - fixture.expected).abs() <= 0.00001);
// Task 4: no retained finite animation after deadline.
assert!(!settled.active);
assert_eq!(settled.next_wake_ms, None);
// Task 5: resolved selection timing and explicit opt-out.
assert_eq!(selection_motion.duration_ms, 220);
assert_eq!(selection_motion.easing, Easing::Spring);
assert_eq!(disabled_motion.duration_ms, 0);
// Task 6: real source parity and component count.
assert_eq!(native_view, javascript_view);
assert_eq!(catalog.len(), 44);
```

```javascript
// Task 3: previous state and active presentation survive a failed patch.
assert.deepEqual(handle.getModel(), before.model);
assert.deepEqual(presentation.sample(now), before.frame);
assert.equal(updateCalls, before.updateCalls);
assert.equal(viewCalls, before.viewCalls);
assert.equal(pendingFrames, 0); // after disposal
```

This is plan 1 of 3. Plans 2 and 3 are part of the same proposed implementation
scope and each has its own release verification. No implementation is authorized
by this document alone: user reviews the written plans and selects native or
subagent-driven execution before coding.
