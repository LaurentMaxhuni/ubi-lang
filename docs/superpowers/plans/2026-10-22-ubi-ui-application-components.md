# Ubi UI Application Components Implementation Plan

## Execution status, 2026-10-23

Tasks 1-5 implemented: overlay contracts, cross-host focus/input, six overlays, selection/menus and application composites. Task 6 release checks passed within the documented host coverage.

User approved all three plans and execution. All subagents used GPT-6 Luna at max reasoning, as requested. The detailed sequences below are the original approved procedure, retained as reference rather than an outstanding work queue. Intermediate catalog checkpoints and proposed per-task commit messages were consolidated around the shared host implementation. The authoritative final results and remaining platform limits are in [verification](../../ubi-ui-motion-verification.md). Physical mobile/native OS accessibility checks remain outside verified coverage.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add 15 usable Ubi application components with shared overlay, selection, focus and semantic behavior.

**Architecture:** Extend the validated shared view contract, then compose `.ubi` controls over host-owned overlay/focus machinery. Reuse motion from [plan 1](2026-10-22-ubi-ui-motion.md); logical closure and semantic changes precede visual exit.

**Tech Stack:** Existing Ubi source modules, Rust core/layout/input/render/platform, JavaScript DOM host, Node/Rust tests and live Next documentation.

**Spec:** [Approved motion/component design](../specs/2026-10-22-ubi-ui-motion-design.md). Prerequisite: plan 1 delivered and verified at 44 catalog components.

## Global Constraints

- Build original, source-owned `.ubi` components for the existing browser and Rust hosts. React remains documentation infrastructure.
- Existing constructor signatures and imports stay available. Schema-1 views without new properties remain valid.
- Every modal has Label or LabelledBy. The active modal traps focus; content beneath it is inert.
- Escape dismisses only the top eligible overlay. AlertDialog requires an explicit action.
- Outside dismissal requires both pointer-down and pointer-up outside, preventing drag-release accidents.
- Timeout is `0..60000` milliseconds, defaults to zero (no auto-dismiss), and is valid only on Toast.
- Position against logical viewport bounds with 8px clearance; flip to the opposite side when the preferred side cannot fit, then clamp.
- Permit at most 4096 live nodes and 4096 visual exit nodes per mount. No shell/network/file authority from controls.
- All new application components require both hosts, real examples, meaningful interaction and reduced-motion behavior before export.

## Review Focus

1. Closing a nested overlay must return focus to an available trigger, not a removed/disabled element (Task 2).
2. Trigger text edits and IME composition must not accidentally activate a menu option or submit (Task 4).
3. Right-click/Shift+F10 near viewport edges must anchor correctly without swallowing unrelated shortcuts (Tasks 2 and 4).
4. Sorting duplicate cell text and navigating a disabled/empty option set must preserve stable source keys (Tasks 4–5).
5. Timeout and focus events queued after close/dispose must not dismiss a newly reopened toast/dialog (Tasks 2–3).

## File map

- `ubi-ui/packages/ubi-ui/core.ubi`, Rust core `src/lib.rs` / new `src/overlay.rs`, browser `validate.mjs`: overlay ABI and semantic relationships.
- New Rust layout `src/overlay.rs` and input `src/overlay.rs`: placement, focus and dismissal using existing Scene/Event types.
- New `ubi-ui/web/overlays.mjs`: browser overlay stack, focus and timers; renderer delegates to it.
- One `.ubi` module and `ubi-ui/examples/components/<slug>.ubi` for each named component below; facade/catalog/docs grow only with completed behavior.
- `ubi-ui/fixtures/overlays.json` (new), crate tests, `ubi-ui/web/tests/overlays.test.mjs` (new), browser regressions and native/web source tests: shared behavior evidence.

## Task 1: Overlay ABI and typed source helpers

**Files:** modify shared core types and both validators; create Rust `src/overlay.rs`, `tests/overlay.rs`, shared `fixtures/overlays.json`; extend `web/tests/validate.test.mjs`.

**Interfaces:** add exactly the spec's Overlay/MenuItem/OptionItem kinds, OverlayKind/Placement enums and Open/Anchor/Placement/Dismiss/TimeoutMs/LabelledBy/DescribedBy properties. Add `Prop.OpenRequest(M)` for host-triggered tooltip/context requests; it dispatches existing Event.Activate and is valid only on Tooltip/ContextMenu overlays with Anchor. Rust normalized `OverlaySpec` owns these decoded values; `validate_overlays(view: &View) -> Result<(), Error>` checks relationships after full graph decoding. Browser validator performs the same postpass.

Add shared `MenuAction<M> { value: string, label: string, message: M, disabled: bool }`, `TableColumn { key: string, label: string, sortable: bool }`, `TableRow { key: string, cells: List<string> }` source records. Menu items store value in Prop.Option; table headers/rows store column/row key in Prop.Option. These values must be nonempty and unique within their owner; normalized host semantic validation rejects duplicates before commit. Extend Role's allowlist only for implemented overlay/menu/listbox/table semantics.

- Add matching `overlay_relationships_and_bounds`: Label/LabelledBy rules, dangling/self label/description/anchor references, duplicate menu values, wrong property-kind pairs, cyclic overlay ancestry, TimeoutMs -1/60001, default Open=false and exact placement defaults. Fixture assertion: `assert_eq!(decoded.open, false)`; invalid cases assert `View::decode(raw).is_err()` and `assert.throws(() => validateView(raw))`.
- Run `rtk cargo test --locked -p ubi-ui-core --test overlay` and `rtk proxy node --test ubi-ui/web/tests/validate.test.mjs`; verify unsupported-ABI failures first.
- Implement decoders and semantic validation without weakening existing Role/codec bounds. Preserve source keys; assign browser IDs only inside a mount.
- Repeat checks and existing `native_components` / `web_ui` tests; commit as `feat(ui): validate overlay and selection contracts`.

## Task 2: Cross-host overlay stack, positioning and focus

**Files:** create layout/input overlay modules and corresponding tests; create `web/overlays.mjs`, `web/tests/overlays.test.mjs`; modify renderer, native host/painting, export allowlist and browser regression harness.

**Interfaces:** layout `place_overlay(anchor: Rect, size: (f32,f32), viewport: Rect, placement: Placement) -> Rect` implements flip/clamp/8px rule. If viewport is smaller than 16px, constrain to nonnegative available space and clip; never emit NaN/negative sizes. Input exports OverlayStack methods `sync(&mut self, view: &View, scene: &Scene)`, `escape(&mut self, view: &View) -> Option<Event>`, `pointer_down(&mut self, x: f32, y: f32)`, `pointer_up(&mut self, view: &View, x: f32, y: f32) -> Option<Event>`, `request_context(&mut self, view: &View, anchor_key: &str, pointer: Option<(f32,f32)>) -> Option<Event>` and `restore_focus(&mut self, view: &View) -> Option<String>`. Internally preserve opening order, source trigger key, mount-local focus history and per-opening generation. Focusable fallback is the nearest surviving parent trigger, then first eligible control in the active modal, then the mount root.

Browser `createOverlayStack(root, dispatch)` returns matching sync/dismiss/dispose behavior plus DOM-specific focus containment and aria/inert wiring. `dispatch(key, message)` is the renderer's existing validated revision-aware activation path. Paint overlay layers above normal scene in stack order. Closed overlays leave hit/focus/semantics immediately; retained visual exits reuse plan 1.

- Add `topmost_modal_focus_and_dismissal`: two open layers, Escape closes only the eligible top; AlertDialog ignores Escape/backdrop; removing/disabling its trigger selects the documented fallback. Assert `events.length === 1`, `assert.equal(document.activeElement, survivingTrigger)`, and `assert_eq!(input.hit(...), None)` for underlying modal-inert content.
- Add `outside_pointer_pair_and_anchor_edges`: inside-down/outside-up never dismisses; outside-down/outside-up dismisses once; scroll/resize/animated-anchor movement recomputes placement; tiny viewport remains finite/clipped. Right-click position and Shift+F10 use the correct pointer/anchor bounds.
- Add `stale_overlay_generation_is_ignored`: queued callback from a closed generation cannot act on its reopened replacement, and disposal removes all listeners/timers. Assert `assert.equal(reopenedModel.open, true)` after delivering the stale callback.
- Run focused Rust layout/input tests and Node overlays tests, then browser harness; confirm failures before integration.
- Implement stack/placement/focus as host infrastructure, with source model controlling actual open/closed state. Trap Tab/Shift+Tab only in active modals; Tooltip has no focusable content. Do not mutate application model in layout/paint.
- Repeat tests plus native/GPU window checks; verify CSS portals remain within mount ownership and style isolation. Commit as `feat(ui): manage overlays focus and viewport placement`.

## Task 3: Six overlay components with source-owned examples

**Files:** create component/example files for `dialog`, `alert-dialog`, `drawer`, `popover`, `tooltip`, `toast`; modify facade, catalog, component notes, renderer effect styling and source/host tests.

**Interfaces:** internal source `overlayComponent<M>(key: string, label: string, kind: OverlayKind, open: bool, dismiss: M, props: List<Prop<M>>, children: List<View<M>>) -> View<M>` owns Overlay kind/Open/Label/Dismiss and appends caller props without duplication. Public `dialog<M>`, `alertDialog<M>`, `drawer<M>`, `popover<M>` and `toast<M>` each use `(key: string, label: string, open: bool, dismiss: M, props: List<Prop<M>>, children: List<View<M>>) -> View<M>`; Popover requires caller Anchor. `tooltip<M>(key: string, anchor: string, content: string, open: bool, request: M, dismiss: M, props: List<Prop<M>>) -> View<M>` owns Tooltip, Anchor/OpenRequest and the trigger description relationship. AlertDialog's dismiss message is used only by explicit cancel source control.

Tooltip requests opening on trigger focus immediately, on pointer hover after 350ms, dismisses on blur/leave/Escape, and ignores touch hover emulation. Suppress duplicate opening requests within a generation. Toast timeout measures visible unpaused time, pauses on hover/focus/hidden host, resets on a new opening generation, and emits Dismiss once; zero means no timer. Source examples include visible close controls and a safe initial AlertDialog cancel action.

- Add `overlay_examples_roundtrip`: native/JS init/update/view agree; controlled open and dismiss messages close the intended component. Assert catalog count 50 and every new example owns a distinct source module.
- Add `tooltip_focus_touch_and_timeout` and `toast_pause_reopen`: assert no tooltip request at 349ms, one at 350ms, immediate focus request, no touch-hover request; toast focus/hover pauses remaining time, close/reopen invalidates old timer; `assert.equal(dismissCount, 1)` after expiry and repeated ticks.
- Run failing source/host cases, implement the six modules and host presentation details, then rerun native/web/browser and sampling/GPU checks in full/reduced modes.
- Publish only working entries/previews and commit as `feat(ui): add dialogs drawers popovers tooltips and toasts`.

## Task 4: Selection, menus and CommandPalette

**Files:** create component/example files for `select`, `combobox`, `dropdown-menu`, `context-menu`, `command-palette`; extend shared input and browser overlay modules, facade/catalog/docs and regression tests.

**Interfaces:** source constructors, with every generic message type `M` and props `List<Prop<M>>`:

```ubi
select<M>(key: string, label: string, choices: List<Choice>, selected: string, open: bool, request: M, change: M, dismiss: M, props: List<Prop<M>>) -> View<M>
combobox<M>(key: string, label: string, choices: List<Choice>, selected: string, query: string, open: bool, request: M, input: M, change: M, dismiss: M, props: List<Prop<M>>) -> View<M>
dropdownMenu<M>(key: string, label: string, actions: List<MenuAction<M>>, open: bool, request: M, dismiss: M, props: List<Prop<M>>) -> View<M>
contextMenu<M>(key: string, anchor: string, label: string, actions: List<MenuAction<M>>, open: bool, request: M, dismiss: M, props: List<Prop<M>>) -> View<M>
commandPalette<M>(key: string, label: string, actions: List<MenuAction<M>>, query: string, open: bool, input: M, dismiss: M, props: List<Prop<M>>) -> View<M>
```

Choice selection emits Event.Select(change, value); per-action menus emit Event.Activate(action.message). Trigger opening is Event.Activate(request). Query changes use Event.Input(input, text). Filter labels deterministically in pure Ubi using lowercase substring matching; keep original values/keys and order. Source fixtures cover Unicode without claiming locale-sensitive matching. Host keyboard logic navigates enabled choices with arrows/Home/End; typeahead buffer expires after 500ms; Escape dismisses. No component registers global Cmd/Ctrl+K; application triggers commandPalette explicitly. ContextMenu intercepts context/Shift+F10 only on its own anchor. Native input needs a context-request path and modifier-aware F10 mapping, without changing existing key meaning elsewhere.

- Add `selection_filter_and_disabled_roundtrip`: empty/all-disabled sets activate nothing, duplicate labels preserve distinct values, no match displays a labelled empty state, selected value persists when filtered out. Assert `assert_eq!(selected_value, "second-stable-key")` after selecting the second duplicate label.
- Add `ime_and_menu_keyboard`: composition updates do not trigger selection; Enter while composing activates nothing; committed query filters once; arrows skip disabled items; Home/End/typeahead focus the expected value; context shortcuts outside anchors remain unhandled.
- Run focused failing Ubi/native/Node/browser tests before extending input, constructors and examples.
- Implement shared selection behavior and source-composed trigger/list surfaces. Editable Combobox retains native input focus/caret and uses active-descendant semantics for list navigation; pointer selection cannot lose the pending query before commit.
- Repeat both host suites and verify command selection/focus return with full/reduced motion. Assert catalog 55; commit as `feat(ui): add selects searchable menus and command palette`.

## Task 5: Pagination, Stepper, Timeline and Table

**Files:** create component/example files for `pagination`, `stepper`, `timeline`, `table`; extend core kinds/role validation and native/browser table rendering; update facade/catalog/docs and tests.

**Interfaces:** add Kind.Table/TableRow/TableHeader/TableCell and validated roles `table`, `row`, `columnheader`, `cell`; Table/Row are containers and Header/Cell hold source children. Shared header/cell width calculation aligns rows and honors scrolling/clipping. Constructors:

```ubi
pagination<M>(key: string, current: int, total: int, previous: M, next: M, props: List<Prop<M>>) -> View<M>
stepper<M>(key: string, label: string, choices: List<Choice>, selected: string, change: M, props: List<Prop<M>>) -> View<M>
timeline<M>(key: string, props: List<Prop<M>>, children: List<View<M>>) -> View<M>
table<M>(key: string, label: string, columns: List<TableColumn>, rows: List<TableRow>, sort: M, props: List<Prop<M>>) -> View<M>
```

Pagination has previous/next actions plus current/total text; current is one-based. Add container `Kind.Pagination` with integer `Prop.Page(int)` / `Prop.PageCount(int)` valid only on this kind, each bounded `1..100000`. Host validation rejects `current > total` before commit. Stepper uses controlled Select events through OptionItem with validated button semantics. Timeline uses ordered list/listitem role mappings with each child's own readable title. Table headers emit Event.Select(sort, column.key) only when sortable; actual row sorting occurs in the Ubi reducer. Reject row-width mismatches, duplicate/empty keys and more than 64 columns before committing a table; retain overall graph budgets. This Table is for bounded data, with no implied virtualization or editable cells.

- Add `pagination_boundaries`: assert previous disabled at 1, next disabled at total, and invalid states reject without losing previous view. `assert!(decode_page(0, 10).is_err())` names a local test helper, not a new public API.
- Add `table_sort_duplicate_text_keeps_keys`: two rows with identical cell text retain different source keys and focus after ascending/descending sort; non-sortable headers emit no event, empty rows render a readable empty state; malformed widths reject. Add Stepper controlled selection and Timeline reading-order/native-semantic tests.
- Run failing native/JS/browser cases; implement source composition and shared table geometry/semantics with the smallest host additions.
- Repeat both host checks, narrow scrolling and GPU alignment capture. Assert catalog 59 and commit as `feat(ui): add pagination stepper timeline and table`.

## Task 6: Application release verification

**Files:** extend `docs/ubi-ui-motion-verification.md`, relevant guide/platform copy, real catalog previews/captures and `PLAN.md`.

**Interfaces:** catalog reaches 59 only after all 15 source modules and required behavior work on both hosts. Exporter uses catalog-derived counts from plan 1.

- Run full release commands from plan 1 Task 7; require all relevant Rust/Node suites, format, Clippy, documentation build and `rtk git diff --check` passing.
- Exercise all 15 real examples in both themes, reduced/full motion and 320/768/1440px widths. Check modal keyboard order, nested layering, timeout/pause, text composition and source copy/reset. Confirm contextual shortcuts do not hijack unrelated controls.
- Check native logical/accessibility snapshots, real window behavior, sampled motion frames and absence of queued work after teardown. Record OS/device/manual limitations explicitly.
- Review all five Review Focus conditions and fix concrete failures, then commit verification as `docs(ui): record application component verification`.

## Handoff

Complete after plan 1 and before [plan 3](2026-10-22-ubi-ui-expressive-components.md).
Execution completed using the approved subagent workflow and final root integration checks.
