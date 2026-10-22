# Ubi UI Expressive Components Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the remaining 11 expressive components, completing the approved expansion from 40 to 70 components.

**Architecture:** Compose source-owned Ubi surfaces and controls over the motion/effect contract from [plan 1](2026-10-22-ubi-ui-motion.md), using shared action/selection types from [plan 2](2026-10-22-ubi-ui-application-components.md). Hosts enhance presentation without changing model/semantic behavior; explicitly document native static fallbacks where a visual enhancement exists only in the browser.

**Tech Stack:** Existing Ubi source, pure motion modules, CSS/DOM presentation, Rust GPU primitives, shared catalog and real documentation previews. No shader library, image pipeline or third-party animation framework.

**Spec:** [Approved motion/component design](../specs/2026-10-22-ubi-ui-motion-design.md). Prerequisite: plans 1 and 2 delivered and verified at 59 components.

## Global Constraints

- Build original, source-owned `.ubi` components for the existing browser and Rust hosts. React remains documentation infrastructure.
- Existing constructor signatures and imports stay available. Schema-1 views without new properties remain valid.
- Pointer effects require useful touch/keyboard behavior. Focus rings remain immediate, visible and unclipped throughout animation.
- User preference changes settle finite transitions immediately and stop decorative loops.
- Components remain understandable without movement; no essential content depends on a reveal, pointer proximity or autoplay.
- Expressive components require the same semantic behavior and declared static fallback on both hosts; renderer-specific visual enhancement is labelled explicitly.
- Do not advertise shader, image loading or true 3D support through decorative names.
- Every component needs a real interactive documentation preview, copyable Ubi source, light/dark styling and narrow-screen behavior.
- Permit at most 4096 live nodes and 4096 visual exit nodes per mount; retain native frame primitive budgets and per-node channel bounds.

## Review Focus

1. Pointer movement over a link/button inside a card must never steal or delay activation (Task 1).
2. Removing the active stack/carousel item must not leave focus in an invisible panel or corrupt selection (Task 2).
3. Auto-rotation after pause/reopen/hide must not dispatch stale or duplicate advance messages (Task 3).
4. Repeated visual copies must not duplicate IDs, accessible content or tab stops (Tasks 3–4).
5. Nested/initially hidden scroll containers must reveal content once at the correct viewport and remain usable without motion (Task 5).

## File map and common interfaces

- Extend `core.ubi` Effect with SpotlightCard, TiltCard, CardStack, Carousel, Dock, Marquee, RotatingText, BorderBeam, AuroraBackground, GridBackground and ScrollReveal. Extend Rust/browser validators in the task owning each effect; no unsupported exports in advance.
- Add typed `Prop.Index(int)`, `Prop.AutoPlay(bool)`, `Prop.PeriodMs(int)`, `Prop.Paused(bool)` and `Prop.Advance(M)` only when the owning task needs them. Internal host action timers emit existing Event.Activate(Advance); frame sampling still never invokes reducers.
- Create `ubi-ui/web/effects.mjs` and `ubi-ui/crates/ubi-ui-render/src/effects.rs` for drawing/effect target calculations. They consume shared MotionFrame, source Node and presented bounds; no compiler/runtime invocation.
- Add `ubi-ui/crates/ubi-ui-core/tests/effects.rs`, `ubi-ui/web/tests/effects.test.mjs` and shared `ubi-ui/fixtures/effects.json` for bounds/semantic tests. Extend native/browser interaction and source roundtrip tests.
- Each effect has a module in `packages/ubi-ui/components/<slug>.ubi`, a real `examples/components/<slug>.ubi`, facade export, catalog entry and component notes. All repository-relative paths in this plan are beneath `ubi-ui/` unless prefixed with `tests/` or `docs/`.

## Task 1: SpotlightCard, TiltCard and Dock

**Files:** create `spotlight-card.ubi`, `tilt-card.ubi`, `dock.ubi` in the component/example directories; create effect drawing modules and tests; modify core/validators, both renderer/input integrations, facade/catalog/docs.

**Interfaces:**

```ubi
spotlightCard<M>(key: string, props: List<Prop<M>>, children: List<View<M>>) -> View<M>
tiltCard<M>(key: string, props: List<Prop<M>>, children: List<View<M>>) -> View<M>
dock<M>(key: string, label: string, actions: List<MenuAction<M>>, props: List<Prop<M>>) -> View<M>
```

Each owns EffectSurface/Effect and keeps child keys unchanged. Card pointer coordinates normalize to `[0,1]` over presented bounds; zero-size surfaces produce no targets. Web TiltCard is bounded to ±6 degrees with 220ms Spring return. Native initially uses its documented static card treatment, preserving actions and semantics; do not invent true-3D claims. SpotlightCard web highlight uses a 160px radial falloff; native uses bounded gradient quads or a clearly documented static highlight. Dock's visual scale is at most 1.35; each underlying target retains at least 44×44 logical pixels and source order. Keyboard focus gets a stable highlight; touch and reduced motion use static controls. No pointer capture for any decorative effect.

- [ ] Add `card_effects_preserve_child_actions`: nested controls receive exactly one action while pointer feedback changes; zero-size surfaces stay finite; reduced/touch input creates no continuous tracks. Fixture assertions include `assert.equal(actionCount, 1)`, `assert.ok(scale <= 1.35)` and `assert_eq!(frame.active, false)` in static mode.
- [ ] Run Rust effects/input tests, `rtk proxy node --test ubi-ui/web/tests/effects.test.mjs` and browser harness; confirm failures before adding effects.
- [ ] Implement validated effect-kind pairs, original visual treatments and three Ubi examples; keep native/browser support descriptions explicit.
- [ ] Rerun tests, keyboard/touch-like browser checks, native semantic/static captures and full/reduced visual review. Assert catalog 62 and commit as `feat(ui): add spotlight tilt cards and dock`.

## Task 2: CardStack and Carousel

**Files:** create `card-stack` and `carousel` components/examples; extend effect modules, shared validators/input and facade/catalog/docs/tests.

**Interfaces:** both constructors use `(key: string, label: string, selected: int, previous: M, next: M, props: List<Prop<M>>, children: List<View<M>>) -> View<M>`. Public names are `cardStack<M>` and `carousel<M>`. Add Prop.Index, default zero; valid range is `0..children.length-1`, with only zero valid for empty content. Previous/next are source-owned buttons and disabled at boundaries. Source reducer owns selection. Index applies to content children of the effect surface, excluding wrapper navigation controls.

Only active content is interactive/semantic; inactive stack cards may be decorative. Carousel defaults to no autoplay; automatic behavior is not exposed in this task. Child views use existing content controls, not a new image loader. Shared 220ms Spring sampling handles stack positions and carousel translation. Focus moves to an available navigation control before the active child becomes inert/removed; retain native/browser semantics equally.

- [ ] Add `stack_carousel_controlled_selection`: index updates only after an explicit previous/next event; boundary/empty states emit no action; out-of-range Index rejects transactionally; removing the active child while updating Index preserves focus and stable surviving keys. Assert `assert.equal(activeFocusablePanels, 1)` and no action after 10 seconds with no user input.
- [ ] Add `carousel_interrupt_and_resize`: reverse halfway through, verify visible-position continuity; resize in motion updates geometry without replaying entrance; reduced mode immediately presents the selected content.
- [ ] Run focused failing native/JS/browser cases; implement source wrappers and effect geometry without creating visual duplicate controls.
- [ ] Verify both hosts and real previews in narrow layouts; assert catalog 64 and commit as `feat(ui): add controlled card stack and carousel`.

## Task 3: Marquee and RotatingText with explicit pause

**Files:** create `marquee` and `rotating-text` components/examples; extend motion/effect scheduling, core/validators and facade/catalog/docs/tests.

**Interfaces:**

```ubi
marquee<M>(key: string, paused: bool, toggle: M, props: List<Prop<M>>, children: List<View<M>>) -> View<M>
rotatingText<M>(key: string, phrases: List<string>, selected: int, advance: M, paused: bool, toggle: M, props: List<Prop<M>>) -> View<M>
```

Add AutoPlay=false, Paused=false, PeriodMs=3000 and Advance. PeriodMs is `250..60000`; these properties are valid only on RotatingText, except Paused also allowed on Marquee. Advance is required only when AutoPlay=true. RotatingText requires nonempty phrases; source produces one Text child per phrase, with validated Index. Automatic advance emits one Event.Activate per visible unpaused period, never frame-driven calls. Both source helpers include a labelled Pause/Resume button using Event.Activate(toggle). Marquee uses a 30 logical-pixel/second decorative loop when not paused; explicit zero-duration Motion or reduced preference makes it static. Automatic rotation also pauses on hover/focus/hidden host and under reduced motion.

- [ ] Add `paused_hidden_rotation_has_no_stale_events`: at 2999ms advance count is zero, at 3000ms it is one; hovering/focus/pause suspends remaining visible time; resuming never catches up missed periods; dispose/reopen ignores old generation callbacks. Assert `assert.equal(advanceCount, 1)` after replaying an old callback and `assert.equal(pendingTimers, 0)` after dispose.
- [ ] Add `marquee_single_semantic_copy`: visual repeats contain no IDs, inputs, event handlers, semantic nodes or focusable children; original content has one semantic copy. User pause/reduced preference leaves all content readable and schedules no animation.
- [ ] Run focused failing pure/native/browser tests; implement source controls, bounded repetition and visibility/preference-aware timers. If content is wider than viewport, static mode permits ordinary scrolling instead of hiding text.
- [ ] Verify both hosts, pause buttons and native static/animated support notes; assert catalog 66 and commit as `feat(ui): add pausable marquee and rotating text`.

## Task 4: BorderBeam, AuroraBackground and GridBackground

**Files:** create `border-beam`, `aurora-background`, `grid-background` components/examples; extend effect modules, scoped CSS/native painter and facade/catalog/docs/tests.

**Interfaces:** constructors `borderBeam<M>`, `auroraBackground<M>` and `gridBackground<M>` all use `(key: string, props: List<Prop<M>>, children: List<View<M>>) -> View<M>`. Decorative surface itself has no role/name/tab stop; content children retain their semantics. Effect geometry remains inside the declared bounds; no unbounded per-pixel work or newly loaded images/fonts/shaders.

Use a 4000ms border cycle, 12000ms aurora cycle and 6000ms grid-highlight cycle. These are ambient effect periods, independent of finite Motion duration bounds. Explicit zero-duration Motion disables the loop. Reduced/hidden/offscreen output is static. Web uses bounded gradients; native may use static multiquad gradient/grid treatments, documented per component. Cap decorative native geometry at 256 quads per surface and keep the existing whole-frame limit. Empty/zero-size surfaces generate no effect work.

- [ ] Add `ambient_effect_budget_and_semantics`: frame decoration adds no semantic nodes or hit targets; generated quads never exceed 256 per surface; zero-size/reduced/hidden/explicit-zero states schedule no ambient frames. Assert `assert!(effect_quad_count <= 256)` and `assert.equal(extraTabStops, 0)`.
- [ ] Add `ambient_mount_isolation`: disposing one mount stops its work without resetting a neighboring mount; references/IDs stay mount-local and no global CSS resets leak to host pages.
- [ ] Run failing effects/GPU/browser cases; implement bounded drawing and static fallbacks, maintaining legible text and visible focus outlines.
- [ ] Verify both themes, native outputs and reduced/full browser motion; assert catalog 69 and commit as `feat(ui): add border beam aurora and grid surfaces`.

## Task 5: ScrollReveal

**Files:** create `scroll-reveal` component/example; extend browser presentation/effects and native layout/host visibility tracking; update facade/catalog/docs/tests.

**Interfaces:** `scrollReveal<M>(key: string, props: List<Prop<M>>, children: List<View<M>>) -> View<M>` creates EffectSurface/ScrollReveal with default Rise entrance and 220ms EaseOut. Trigger once per key/kind lifecycle when visible intersection has positive area within its nearest scroll container and viewport after clipping. Stable-key reorders/resizes do not replay. An initially hidden container does not consume the reveal; its first visible intersection does. No source state or timers required. Focus entering unrevealed content settles its entrance immediately so the focus indicator remains visible.

- [ ] Add `nested_scroll_reveals_once`: outer viewport visibility alone does not trigger content clipped by inner scroll; positive inner intersection triggers once; theme/resize/reorder do not replay; hide→show triggers only if never revealed. Assert `assert.equal(entranceStarts, 1)` after repeated scroll/resize.
- [ ] Add `scroll_reveal_keyboard_and_reduced`: keyboard focus makes child immediately visible/focused, reduced mode shows content before intersection, and logical layout/semantics never depend on reveal progress. Assert `assert.equal(document.activeElement, child)` and presented opacity 1 after focus.
- [ ] Run failing native layout/input and browser tests; implement scroll visibility as host presentation bookkeeping, with one observer per mount on web and existing Scene clipping on native.
- [ ] Verify real nested-scroll examples in both hosts and docs; assert catalog 70 and commit as `feat(ui): add accessible scroll reveal`.

## Task 6: Whole-expansion verification and delivery

**Files:** update `docs/ubi-ui-motion-verification.md`, `docs/ubi-ui-design-references.md` implementation-status notes, current platform/theming documentation, `PLAN.md`, and real preview/capture outputs.

**Interfaces:** all 70 catalog entries point to existing source modules/examples; old facade imports and 40 original examples remain valid. The catalog lists actual capabilities and static fallbacks rather than implying cross-platform visual parity where none exists.

- [ ] Run full release commands in plan 1 Task 7 and `rtk git diff --check`; expect required suites/builds passing. Verify every real source example compiles and native/JS reducers agree. Catalog assertions use `assert_eq!(catalog.len(), 70)` at final checkpoint, and export manifest is catalog length plus gallery.
- [ ] Inspect the final gallery as a coherent visual system: typography, spacing, contrast, responsive layout, motion restraint, interrupted animation, focus and source copy. Exercise all 30 new examples and original-control regressions in light/dark and reduced/full modes.
- [ ] Verify Browser at 320/768/1440px, native sampled frames/window interaction, no idle work from finite effects, no duplicate semantics and no live callbacks after disposal. Record animation timing and actual host coverage; do not infer iOS/Android/native screen-reader readiness from these tests.
- [ ] Review final branch against the approved spec and all three Review Focus lists, fix concrete issues, and rerun affected checks. Refresh the local code graph for changed-source navigation; keep generated index uncommitted.
- [ ] Commit exact evidence/status as `docs(ui): verify expanded animated component library`. Report delivered counts, commits, test results and remaining platform limitations without claiming unsupported features.

## Handoff

User reviews all three linked plans and selects execution method before code.
Native execution keeps the tightly shared host contracts in one context;
subagent-driven execution adds a fresh implementer/reviewer per task at higher
context cost. Keep meaningful commits in either method.
