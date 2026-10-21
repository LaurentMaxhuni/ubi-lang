# Ubi UI Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship editable Ubi components and a working themed explorer using the same source on web, desktop and mobile.

**Architecture:** Ubi defines components, composition, state and messages. A bounded flat view arena crosses the existing concrete export ABI into a small semantic DOM renderer. Existing Electron/Capacitor hosts package this renderer; their APIs never appear in Ubi component signatures.

**Tech Stack:** Existing Rust compiler, Ubi modules, browser DOM/CSS, Node built-in tests, existing Electron/Capacitor versions and locks. No new product dependency in this slice.

**Spec:** [Ubi UI design](../specs/2026-10-21-ubi-ui-design.md)

## Scope and file ownership

This plan implements the first usable vertical slice: text, headings, column/row
layout, card, badge, button, themes, a button/component explorer, and portable
packages. The implementation stays in `examples/ui/` except independent Rust
conformance and example documentation. Its `src/ui/` files are the actual reusable
library source, not showcase-specific component imitations.

The full release remains larger: grid/container, separator, alert, avatar,
spinner/skeleton, controlled forms, composite controls, application blocks and
`ubi ui init/add` each follow with their own behavior and acceptance coverage.
The first slice offers direct editable source, not an installer. Do not add CLI
placeholders or list unfinished controls in the working catalog.

File responsibilities:

- `examples/ui/src/ui/core.ubi`: versioned arena, closed props, fragments and composition.
- `examples/ui/src/ui/components.ubi`: public Ubi component helpers.
- `examples/ui/src/main.ubi`: explorer state, messages, validation, view and transitions.
- `examples/ui/web/codec.mjs`: bounded strict ABI encoding.
- `examples/ui/web/validate.mjs`: graph and component-contract validation.
- `examples/ui/web/renderer.mjs`: keyed DOM rendering, event routing and lifecycle.
- `examples/ui/web/app.js`: import concrete app exports, mount, host error boundary.
- `examples/ui/web/index.html`: metadata, CSP and mount; no component catalog markup.
- `examples/ui/web/style.css`: semantic themes, layout and component states.
- `examples/ui/tests/*.test.mjs`: independent codec/validation assertions using Node.
- `examples/ui/tests/browser.html`, `browser.mjs`: actual DOM/focus/lifecycle checks.
- `examples/ui/desktop/`, `mobile/`: scoped copies of existing packaging adapters.
- `src/ui_tests.rs`: independently authored interpreter/backend contract conformance.
- `tests/ui_package.rs`: independent packaging regression coverage.
- `examples/ui/README.md`, `docs/ui-foundation-verification.md`: commands and measured evidence.

## Global constraints

- Components, layout composition, application state and messages are authored in `.ubi`.
- Schema version 1; at most 4,096 nodes, 64 logical DOM levels, 128 Unicode scalar values per key and 1 MiB UTF-8 encoded view/event/state payload each.
- Preserve existing compiler/runtime limits, including 32 aggregate levels and existing call/step budgets.
- Keep the complete first-release Ubi UI module set within 24 modules; this slice uses three.
- No function-valued prop, capability, raw HTML, inline event source, unrestricted URL/style prop or new compiler syntax.
- No remote font/icon/CDN dependency. Use the system font stack in this slice; custom font licensing is a later explicit addition.
- Default touch targets are at least 44 CSS pixels. Respect reduced motion and forced colors.
- Existing `ubi init`, package code and task app behavior stay unchanged.
- Preserve sandbox/context isolation, relative production assets and exact Electron asset allowlists.
- All subagents, including independent reviewers, must use GPT 6 Luna at Max.

## Review focus

1. Shared aggregate references can expand exponentially during encoding; reject at the byte budget without materializing the whole expansion (Task 3).
2. Stale/detached controls and two simultaneous mounts must not cross-route messages (Task 4).
3. A rejected update/view or interrupted DOM patch must retain committed state and recover the prior UI (Task 4).
4. An imported generic component with no message payload must infer from `View<Message>` context without exposing a generic host wrapper (Tasks 1–2).
5. Packaged module URLs and strict asset allowlists must work without a dev server or extra filesystem authority (Task 6).

## Fixed first-slice interfaces

`core.ubi` exports these nominal types. Variants are closed; unrelated future
variants require schema/validator tests before exposure.

```text
Kind = Column | Row | Text | Heading | Card | Badge | Button
Space = None | Xs | Sm | Md | Lg | Xl | Xxl
Tone = Neutral | Primary | Success | Warning | Danger
ButtonVariant = Primary | Secondary | Outline | Ghost | Destructive
Theme = Light | Dark | System
Prop<M> = Content(string) | Level(int) | Gap(Space) | Tone(Tone)
        | Variant(ButtonVariant) | Disabled(bool) | Busy(bool)
        | Activate(M) | Pressed(bool) | Theme(Theme)
Node<M> = {key: string, kind: Kind, props: List<Prop<M>>, children: List<string>}
View<M> = {schemaVersion: int, root: string, nodes: List<Node<M>>}
Event<M> = Activate(M) | Input(M, string) | Toggle(M, bool)
         | Select(M, string) | Submit(M)
```

Only `Event.Activate` has a rendered binding in this slice. Other event variants
are reserved typed data for subsequent form work, not functioning form controls.

Core exports `node<M>(key: string, kind: Kind, props: List<Prop<M>>) -> View<M>`
and `group<M>(key: string, kind: Kind, props: List<Prop<M>>, children: List<View<M>>) -> View<M>`.
They produce schema 1 and parent-first flattened nodes with child-root references;
they do not change supplied keys. Host validation rejects invalid compositions.

`components.ubi` imports these types and exports:

```text
text<M>(key: string, value: string) -> View<M>
heading<M>(key: string, value: string, level: int) -> View<M>
column<M>(key: string, gap: Space, children: List<View<M>>) -> View<M>
row<M>(key: string, gap: Space, children: List<View<M>>) -> View<M>
card<M>(key: string, children: List<View<M>>) -> View<M>
badge<M>(key: string, value: string, tone: Tone) -> View<M>
button<M>(key: string, label: string, variant: ButtonVariant,
          disabled: bool, busy: bool, message: M) -> View<M>
themed<M>(view: View<M>, theme: Theme) -> View<M>
pressed<M>(view: View<M>, value: bool) -> View<M>
```

`themed` replaces/adds the root's Theme prop without changing keys or node order.
The root must be Column, Row or Card. `pressed` replaces/adds the root's Pressed
prop; that root must be a Button. No component helper calls a host API.

Host exports:

```text
encode(value: unknown) -> string                         // codec.mjs; throws on invalid/budget
validateView(value: unknown) -> ValidatedView             // validate.mjs; throws before DOM
mount(root: Element, app: App, options?: {onError(Error): void}) -> Mount
Mount = {dispose(): void, recover(): void}                // renderer.mjs
App = {initial(), validate(stateJSON), update(stateJSON, eventJSON), view(stateJSON)}
```

`ValidatedView` is an internal frozen normalized record containing root key,
preorder nodes and resolved props. Never pass renderer-generated data directly to
`$ubi_internal_*` exports. App functions are the compiler's public wrappers:
aggregate arguments are encoded JSON strings; return aggregates are decoded JS
values. Runtime enum data uses `{tag, values}` as the existing ABI specifies.

## Task 1: Arena and independent Ubi contract tests

**Files:** Create `examples/ui/src/ui/core.ubi`, `src/ui_tests.rs`; modify `src/lib.rs` only to register the test module.

**Interfaces:** Produces the exact core types, `node` and `group` above. Tests import through a concrete Ubi wrapper; reuse conformance harness helpers from `src/generics_tests.rs` where suitable.

- [ ] Independently author `flat_composition_preserves_keys_and_order`, `message_type_is_preserved_across_imports`, `context_infers_passive_component_message` and `nested_fragments_stay_flat` from the contract. Assert root `settings`, parent-first `[settings,title,save]`, exact child references and exact `{tag,values}` message shape; verify interpreter and generated backend agree. Assert unresolved/incompatible message types are rejected. Tests must not calculate expected values with library helpers.
- [ ] Run `rtk cargo test --lib ui_tests`; confirm failure because the core source/exports are absent.
- [ ] Implement the core in ordinary Ubi. Flatten with existing list operations and preserve deterministic order. Do not raise budgets or introduce compiler special cases.
- [ ] Rerun the focused tests and `rtk cargo fmt -- --check`; commit the working core and tests.

## Task 2: Public Ubi components

**Files:** Create `examples/ui/src/ui/components.ubi`; extend independent `src/ui_tests.rs`.

**Interfaces:** Consumes Task 1; produces the component helpers above. Tests call generic helpers only from concrete Ubi wrappers, with `View<Message>` annotations where needed for inference.

- [ ] Add `components_emit_fixed_semantics`, `themed_replaces_only_root_theme` and `pressed_replaces_only_root_pressed`. Assert Heading emits Level 1–6 data verbatim, text/label Unicode survives, Button emits one Content/Variant/Disabled/Busy/Activate prop, all five variants stay distinct, and theme/pressed changes neither message nor key identity. Invalid heading values remain rejected by host validation, not silently clamped.
- [ ] Run `rtk cargo test --lib ui_tests`; confirm the new component cases fail before implementation.
- [ ] Implement helpers using `node/group`; supply Ubi examples composing an annotated `View<Message>` from passive and active children. No JSX/React/JavaScript component definitions.
- [ ] Rerun focused tests; commit.

## Task 3: Bounded codec and arena validation

**Files:** Create `examples/ui/web/codec.mjs`, `validate.mjs`, `examples/ui/tests/codec.test.mjs`, `validate.test.mjs`.

**Interfaces:** Consumes Task 1 arena. Produces `encode` and `validateView`; schema parsing uses exact closed variants and payload arities, not truthiness/coercion.

- [ ] Author codec tests for unit/null, NaN, infinities, negative zero, Unicode, cycle rejection, own-data-only records, unsupported functions/symbols and shared DAG expansion. Assert exactly 1,048,576 UTF-8 encoded bytes passes and one extra byte fails, including escaped strings and multibyte scalars. Stop writing at the cap; do not stringify a complete oversized aggregate first. Inherited accessors and getters must not execute.
- [ ] Author validator tests accepting exactly 4,096 nodes/64 levels/128-scalar keys and rejecting boundary+1, empty/duplicate keys, unknown schema/kinds/props, duplicate props, missing/multiple parents, cycles, unreachable nodes, root errors, scalar leaves with children and interactive nesting. Heading requires Content/Level (1–6); text requires Content; Badge requires Content/Tone; Button requires all five props, a nonempty label and no children. Container nodes accept Gap; only the root accepts Theme. Props from another kind are invalid. Enforce aggregate/payload depth and bytes before graph traversal; use iterative traversal for the 64-level graph.
- [ ] Run `rtk proxy node --test examples/ui/tests/codec.test.mjs examples/ui/tests/validate.test.mjs`; confirm absent-module failures, then implement the two modules and rerun to pass. Codec depth limit remains 32 and rejects cycles while allowing repeated acyclic references within the byte budget.
- [ ] Commit codec, validator and their passing tests.

## Task 4: Keyed renderer and lifecycle

**Files:** Create `examples/ui/web/renderer.mjs`, `examples/ui/tests/browser.html`, `browser.mjs`.

**Interfaces:** Consumes codec/validator; produces `mount/dispose/recover`. Validate initial/candidate state with `app.validate` (expect `Result.Ok(state)`), then call `view`; update takes an encoded `Event.Activate(message)` and returns a Result. Do not commit candidate model state until view validation and DOM commit succeed.

- [ ] Write actual-browser checks for native Enter/Space/pointer activation once, disabled/busy buttons, text-only injection safety, compatible keyed element/focus preservation, reorder/removal, kind replacement, two mounts, retained detached-button clicks, synchronous reentry and disposal. Assert a removed/disposed control never updates an app and current keys use current messages. Passive nodes cannot steal focus.
- [ ] Add failure tests: update Err/throw, validate Err, view throw, invalid arena and encoding overflow keep previous state/node identity. Fault-inject a DOM patch operation and assert snapshot rollback; fault-inject rollback and assert mount disposal plus `onError`, followed by explicit `recover()` from last committed model. Keep error UI outside the controlled mount. Clean up theme observers/listeners on dispose/replacement.
- [ ] Implement semantic element mapping: Column/Row/Card to div, Text to p, Heading to h1–h6, Badge to span, Button to button with type=button. Match nodes by key and kind; create text with textContent. Disabled or busy means native disabled and busy also sets aria-busy. Optional Pressed maps to aria-pressed; omit it for ordinary buttons. Native click is the single activation path; do not add duplicate key handlers.
- [ ] Implement mount-local generated IDs, binding ownership/revisions, serial transitions, rollback journal and cleanup. Theme is scoped per mount; System follows media changes without a Ubi state transition and removes its media listener on dispose. Catch/report initialization errors without touching another mount.
- [ ] Serve the dependency-free browser harness from a local static server; run every named check in an actual browser and record engine/version. Node DOM doubles cannot substitute for focus/keyboard evidence. Commit after checks pass.

## Task 5: Ubi explorer and theme styles

**Files:** Create `examples/ui/src/main.ubi`, `ubi.json`, `web/app.js`, `index.html`, `style.css`, `README.md`; extend `src/ui_tests.rs` and browser harness.

**Interfaces:** Concrete Message variants `ChooseButtons`, `ChooseComposition`, `ChooseLight`, `ChooseDark`, `ChooseSystem`, `Increment`, `Reset`; State fields `section: string`, `theme: Theme`, `count: int`. Export `initial() -> State`, `validate(state: State) -> Result<State,string>`, `update(state: State,event: Event<Message>) -> Result<State,string>`, `view(state: State) -> View<Message>` and `main() -> string` for compiler/example checks.

- [ ] Author transition tests: initial Buttons/System/0; Increment adds one up to 999; Reset gives 0; non-Activate events return Err; unknown section and counts outside 0–999 fail validate; Increment at 999 returns Err without changing state. Theme/section changes preserve count. Assert a nonempty valid view from every valid section/theme combination using exact representative descriptor expectations.
- [ ] Implement explorer composition entirely in Ubi: title, selected component navigation buttons, theme toolbar buttons, live preview and a Ubi source excerpt using text. Use `pressed` for selected navigation/theme buttons. Do not call the navigation a tablist until full tab keyboard behavior exists.
- [ ] Build Buttons and Composition sections with all variants, disabled/busy examples, a working counter and reset, a workspace summary card and badges. Use stable descriptive keys. At 320 CSS px the navigator stacks above the preview; mobile navigation sheet is deferred until the dialog wave.
- [ ] Implement local CSS tokens from the spec: light/dark colors, measured primary foreground, 4/8/12/16/24/32/48 spacing, 10/16/20 radii, 16px reading/14px control text. Scope styles under the mount. Use system fonts, visible focus, 44px targets, reduced-motion/forced-color rules and safe-area padding. No palette literals in per-component rules. HTML contains only app/error mounts and metadata; JS contains no showcase state machine or component definitions.
- [ ] Add responsive/focus/theme browser cases, including independent mount themes, System change and two themes at 320/768/1440 CSS px. Check 200/400% zoom, keyboard-only operation and contrast of each enabled interaction/text state. Record screenshots/evidence without claiming full WCAG conformance. Document only working components and exact `ubi dev/check/build` commands from the existing CLI.
- [ ] Run focused Ubi and browser checks, commit the functioning web explorer.

## Task 6: Desktop/mobile packages and regression verification

**Files:** Create scoped host files under `examples/ui/desktop/`, `examples/ui/mobile/`, `web/storage.js`, `tests/ui_package.rs`, `docs/ui-foundation-verification.md`; update `examples/README.md`.

**Interfaces:** Consumes the same Ubi entry and web assets through the existing `ubi package --target web|desktop|mobile` command. No new package manifest schema or compiler behavior.

- [ ] Add package tests asserting all three outputs contain identical Ubi modules and local renderer/codec/validator/styles; packaged app.js uses relative module URLs and has no `/__ubi/` URLs. Run the generated public wrappers under Node and verify initial/Increment/view. Assert sandbox, navigation denial and exact asset restrictions remain enabled. Existing task-app package tests continue passing.
- [ ] Reuse existing desktop/mobile adapter code and pinned lockfiles, changing only app branding, explicit UI asset allowlist (`renderer.mjs`, `codec.mjs`, `validate.mjs`) and UI smoke selectors. Retain required storage.js/native-storage.js assets for the existing package contract, but do not import/invoke storage in this pure UI showcase. Document this compatibility detail. Do not install a back-button plugin before overlay components exist.
- [ ] Package all three targets. Launch packaged Electron and Android where available: choose sections/themes, increment/reset, verify offline assets and cleanup after restart. Record OS and browser/WebView engine versions, actual commands and limitations. iOS generation/build remains explicitly unverified without macOS/Xcode; a browser resize does not count as a device test.
- [ ] Run `rtk cargo test --lib ui_tests`, `rtk cargo test --test ui_package`, Node codec/validator checks and the browser harness. Then run the existing complete `rtk cargo test`, `rtk cargo fmt -- --check`, `rtk cargo clippy --all-targets -- -D warnings` and `rtk git diff --check`; preserve full logs and retrieve failures. Existing sandbox loopback/Windows symlink limitations must be reported, not counted as passes.
- [ ] Obtain a fresh independent GPT 6 Luna / Max review of contracts, implementation, security/accessibility and missing tests. Fix material findings, rerun affected checks, update truthful evidence and commit the completed slice.

## Execution handoff

Recommended method: native execution in this session, with one independent GPT
6 Luna / Max reviewer after the slice. The six tasks share a small evolving
contract; one implementer avoids parallel edits to it. No subagent is needed to
start implementation. Review this plan before execution; the component library
and UI runtime described here do not exist yet.
