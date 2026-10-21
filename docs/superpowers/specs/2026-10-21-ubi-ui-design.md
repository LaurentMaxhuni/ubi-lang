# Ubi UI: component system for web, mobile and desktop

Status: first foundation slice implemented after user approval on 2026-10-21.
See [verification evidence](../../ui-foundation-verification.md). Remaining
component waves and installer are not implemented yet.
User direction: prioritize an Ubi-first system in the spirit of shadcn, with
beautiful, composable components for all three platforms. Other framework
adapters follow after the Ubi experience works.

## Product and ownership

Ubi UI consists of source-owned Ubi components, semantic design tokens, a small
DOM renderer, local component distribution and a working component explorer.
Application state, component composition and typed messages live in `.ubi`.
Trusted JavaScript bootstraps the renderer and existing platform adapters.

Authorship boundary: the public component library, composition, state and event
behavior are Ubi source. JavaScript is the current compiler target and the small
platform rendering bridge. Electron/Capacitor supply the app shell; application
authors do not write Electron/Capacitor components. Local CSS supplies editable
theme tokens and renderer styles. Keep host names and host-specific APIs out of
the public Ubi component signatures so a future renderer can reuse them.

Web renders semantic HTML. Desktop uses the existing sandboxed Electron host;
mobile uses the existing Capacitor WebView host. These share the same Ubi
components and CSS. Native UIKit/SwiftUI/Compose controls require separate
renderers and are outside this first release. CLI has no graphical target.

The source-ownership/composition model follows the principles described in
[shadcn's introduction](https://ui.shadcn.com/docs); this is an Ubi implementation,
not a dependency on React, Tailwind, Radix or copied shadcn component code.
Component files and themes remain readable and editable in the user's project.

## Approach chosen for review

| Approach | Result | Tradeoff |
| --- | --- | --- |
| Ubi views/messages plus a shared DOM renderer | Recommended: Ubi owns the component API on all current hosts | Requires a small renderer and a typed view contract |
| Wrap an existing JavaScript framework immediately | Reuses existing ecosystem behavior | Moves component authoring and much of the public API outside Ubi |
| Build three native renderers first | Native OS widgets | Multiplies layout, accessibility and platform work before proving the Ubi API |

No new compiler syntax, unrestricted foreign calls, model judgments or host
authority are needed for the recommended approach. Existing Ubi record/enum/
generic support is sufficient. Do not raise compiler or runtime limits to make
the UI fit.

## Visual direction

The explorer should feel like a polished app-builder's workbench: crisp type,
generous composition, restrained surfaces and vivid blue interaction states.
Show finished component compositions alongside individual states. Avoid a wall
of identical cards or a marketing hero that displaces the usable components.

Use a left component navigator, a compact theme/device toolbar and a large live
stage with Preview/Code tabs. On mobile the navigator becomes a sheet; examples
reflow into a single column. A device switch changes the preview viewport,
not the component implementation. Include both a form/preferences block and a
small workspace block to prove components compose into convincing applications.

Proposed light palette: canvas `#F7F9FC`, surface `#FFFFFF`, ink `#17223B`, muted
ink `#5D6880`, border `#DEE4EF`, primary `#235FEA`. Dark palette: canvas `#101724`,
surface `#182235`, ink `#EFF3FA`, muted ink `#A7B3C9`, border `#344158`, primary
`#89ADFF`. Pair primary colors with separately measured foreground tokens.
Destructive/success/warning colors are semantic tokens, not component literals.
Use a separate control-border token with adequate contrast; the lighter border
token is for decorative surface separation, not the only indicator of an input.

Typography direction: Plus Jakarta Sans, self-hosted only after source/license
verification, with a system sans fallback. Use a 16px reading size and a compact
14px control label size; titles use 24/32/40px steps. Spacing uses 4/8/12/16/24/
32/48px. Radii distinguish controls (10px), surfaces (16px) and overlays (20px).
Shadows distinguish elevation, primarily overlays. Components never hardcode
palette values. Expose theme, radius and density without forking component code.

Default touch targets are at least 44 CSS pixels. An explicit compact density may
use smaller visual controls on fine pointers; touch layouts retain the target
size. Interaction motion is brief and purposeful; respect reduced motion.
Light/dark/system themes, forced colors and visible focus are first-class states.
Measure contrast against WCAG 2.2 AA targets; do not claim conformance from a
palette or automated audit alone.

## Ubi view contract

Use a flat immutable view arena, not a recursive serialized component tree.
Nested enum/record/list wrappers otherwise consume several aggregate levels per
DOM level and exhaust Ubi's existing 32-level aggregate bound too quickly.

Conceptual data model; final exported type names/signatures are fixed in the
implementation plan after design approval:

```text
View<Message> = { schemaVersion: int, root: string, nodes: List<Node<Message>> }
Node<Message> = { key: string, kind: Kind, props: List<Prop<Message>>,
                  children: List<string> }
Event<Message> = Activate(Message) | Input(Message, string)
               | Toggle(Message, bool) | Select(Message, string)
               | Submit(Message)
```

Kinds and props are closed typed enums. Component helpers compose flat fragments
and emit schemaVersion 1. Unknown schema versions are rejected. The helpers
return `View<Message>`; children reference stable keys. Public exported app
functions use concrete State and Message types. No function-valued prop crosses
the public host ABI. Ubi components may use pure helpers internally.

The developer-facing component shape is ordinary Ubi, conceptually:

```ubi
export enum Message { Save, EditName, ToggleDigest }
export record State { name: string, digest: bool }

// Component APIs are proposed here, not implemented or compiler-tested yet.
export fn view(state: State) -> View<Message> {
    column("settings", [
        textInput("name", "Workspace name", state.name, Message.EditName),
        button("save", "Save changes", ButtonVariant.Primary, Message.Save)
    ])
}
```

The input binds Message.EditName; Event.Input carries the new string. The helper
imports and complete prop signatures belong to the reviewed implementation plan.

The application exports concrete, synchronous pure functions:
`initial() -> State`, `validate(State) -> Result<State, string>`,
`update(State, Event<Message>) -> Result<State, string>`, and
`view(State) -> View<Message>`. Validate initial and candidate states before view;
typed errors preserve the committed state and supply readable error text.
Restored/external states require structural ABI validation and application-owned
semantic validation (IDs, versions and domain invariants); type decoding alone
does not establish those invariants.
The renderer invokes the public wrappers and validates typed data through the
existing ABI. It must encode unit and special floats exactly as SPEC14/15 require
(the CLI encoder in src/cli.rs is an existing reference), not silently lose them
with plain JSON.stringify. State/messages may not contain functions, capabilities
or other values forbidden by that ABI.

Proposed UI profile: at most 4,096 nodes, 64 logical DOM levels, 128 characters per
key and 1 MiB encoded view/event/state payload each. Encoding must stop at its
budget rather than materialize an exponentially expanded shared aggregate.
These UI limits supplement, never replace, existing Ubi budgets. Final tests
cover exact boundaries. Keys are unique per mount, stable across updates and
scoped into mount-local DOM IDs. Component internals derive child keys from the
caller-supplied key.

## Renderer and events

Validate a complete candidate arena before changing DOM: schema/version, known
kinds/props, key uniqueness, valid root, child references, cycles, one parent per
node, reachability, size/depth limits and semantic prop combinations. Require
valid label/description/control links. Reject invalid nested interactive controls
and conflicting properties. Validation failures cannot partially replace the UI.

Use text nodes/textContent; there is no raw HTML, arbitrary script, inline event
source or unrestricted URL/style prop. Styles are local CSS and theme tokens.
Initial icons use a local audited set; no network font/icon/image dependency is
required to display the library. Core UI types grant no network/storage/device
authority.

Reconcile by stable keys and node kind. Preserve compatible DOM elements, focus,
selection and native control state; do not replace the whole mount on every
keystroke. Coordinate controlled value writes with compositionstart/end and IME
input. Events generated by reconciliation must not recursively re-enter update.
Use existing native controls before inventing custom keyboard behavior.

Route normalized events from the current committed binding table. Include mount
and revision ownership; ignore events from disposed mounts or removed controls.
Process one transition at a time and resolve duplicate activation deterministically.
An event's application message comes from its declared binding; DOM event objects
and ambient DOM access never become ordinary Ubi values.

Compute candidate state, semantic validation, candidate view and arena validation
before committing model
state. Update/view/encoding/validation failures preserve the committed model and
unrelated visible UI; retain a user's uncommitted editing text and report a host
fault without treating it as a successful transition. An unexpected DOM commit
fault rolls back the patch from its saved node/property snapshot. If rollback
fails, dispose that mount and show a separate error boundary while retaining the
committed model for explicit remount/recovery. Never persist candidate state
before a failed transition. Dispose removes listeners, observers, timers
and platform hooks, including when mounting a replacement app.

Persistent effects are a separate typed command/host-adapter follow-up. The first
showcase uses pure transitions; the existing durable app is not silently migrated
to optimistic writes. Later durable controls commit only after host success and
retain visible/persisted data on failed writes or corrupt loads, per SPEC22.

## Component delivery sequence

Each wave ships working, independently tested behavior before expanding scope.

1. Foundation and explorer: tokens, themes, text/headings, stack/grid/container,
   separator, surfaces/card, badge, alert, avatar, spinner/skeleton and button.
2. Controlled forms: field/label/help/error, input, textarea, checkbox, switch,
   native select, form submission, disabled/busy/invalid states.
3. Composite interaction: tabs, dialog and responsive sheet, tooltip and toast.
4. Reusable blocks: settings form, workspace navigation and searchable collection
   view; example app composition using the same public components.

Do not ship stub controls as implemented catalog entries. Core forms must be
stable before modal/roving-focus behavior. Menus/comboboxes, date pickers, sortable
tables, command palettes, drag/drop, charts and full navigation/routing are later
waves driven by complete accessibility/interaction tests. Popular framework
adapters reuse the proven tokens/contracts only after Ubi authoring is established.

## Component interaction requirements

- Buttons use native button semantics, an explicit type, accessible name and
  disabled/busy behavior; Enter/Space/pointer produce one transition.
- Fields use visible labels and linked help/errors. Test typing, paste, emoji,
  selection, dead keys, composition and rerenders. Unknown select values fail
  validation rather than silently choosing another option.
- Checkbox/switch labels toggle once; semantics and disabled state are announced.
- Tabs link tablist/tab/tabpanel IDs, maintain one selection, support roving focus,
  arrows/Home/End and RTL. Use manual activation by Enter/Space initially.
- Dialog/sheet uses the native modal primitive where supported: named content,
  deliberate initial focus, inert background, contained tab order, Escape/close,
  invoker focus restoration and cleanup. Sheet is a responsive dialog variant.
  Android back dismisses the top overlay before requesting navigation.
  The current template has no App-plugin back listener; implementing this wave
  must add a scoped [Capacitor App adapter](https://capacitorjs.com/docs/apis/app)
  or equivalent native bridge and verify listener disposal. This is a planned
  host dependency, not an existing capability or an automatic installation.
- Toast uses a persistent polite live region, urgent alert only for urgent
  failures, accessible dismissal/actions and no focus theft. Pause dismissal on
  hover/focus; respect reduced motion. Explicit lifetime/cleanup rules precede
  its implementation.

The keyboard/modal details follow W3C's [tabs pattern](https://www.w3.org/WAI/ARIA/apg/patterns/tabs/)
and [modal dialog pattern](https://www.w3.org/WAI/ARIA/apg/patterns/dialog-modal/).
The quality target is [WCAG 2.2 AA](https://www.w3.org/WAI/WCAG22/quickref/), plus
the stronger touch-target design rule above. Validate 320 CSS px reflow, 200/400%
zoom, both themes, forced colors, reduced motion, virtual keyboard, portrait/
landscape, safe areas, keyboard-only use and available screen readers.

## Source distribution and platform integration

Proposed local commands: `ubi ui init` installs the small core/theme/bootstrap;
`ubi ui add button input ...` copies requested Ubi components and their declared
dependencies. Names and exact manifests are fixed in the next implementation plan.
No remote registry, dependency installation, publishing or telemetry by default.

Copies remain inside the project root. Validate all destinations/collisions before
writing, preserve user-owned code and refuse silent overwrite. User edits never
disappear on a repeated add/init. Report exact integration changes; migrations and
upstream updates require explicit reviewed diffs. The current `ubi init` template
and existing applications retain their behavior until a UI option is selected.

Package web/desktop/mobile with the same versioned source, renderer, tokens and
relative assets. Reuse existing CSP, sandbox, context isolation, navigation denial
and scoped host adapters. No CDN/dev-server dependency in production output.
The compiler's root-relative import rules apply to copied component sources.
Keep the complete first-release Ubi UI module set within 24 modules, leaving room
inside the existing 64-module project budget; group related exports where useful.
The current Electron protocol allowlist only permits its named assets and
`modules/*`: add each required UI asset explicitly or bundle into `app.js`.
Do not broaden it into arbitrary filesystem serving.

## Definition of the first release

The explorer itself is an Ubi UI app, with editable source examples, light/dark/
system themes, real working controls and responsive web/mobile/desktop previews.
It must not be a JavaScript-only gallery branded as Ubi.

Independent tests derive expected view descriptors, state transitions and invalid
cases from the approved contract. Browser tests prove focus/caret/composition,
keyboard/touch semantics, labels, theme contrast, failures and graph boundaries.
Installer tests cover collisions, traversal, symlinks, hardlinks, interrupted
writes and repeated commands. Preserve all compiler conformance and budgets.

Record actual interaction and package evidence separately for browser, packaged
Electron and Android WebView; iOS WKWebView checks require the available Apple
host/device. A browser resize is a layout check, not a native package check.
Record the actual WebView/browser engine version as well as the OS version;
an Android/iOS support label alone does not establish renderer compatibility.
Missing platforms, screen readers and signing credentials remain explicit.

## Review handoff

The user delegated the platform-host choice after clarifying that components
must be written in Ubi. Retain the shared DOM approach for the first release;
the authorship boundary above is mandatory. The first implementation plan covers
the foundation/explorer vertical slice and its web/desktop/mobile packaging.
Remaining foundation components, forms, composites, blocks and local copy-in CLI
follow after that slice; none are claimed implemented by this document.
Review the executable plan before production implementation. All subagents remain
GPT 6 Luna at Max per user instruction.
