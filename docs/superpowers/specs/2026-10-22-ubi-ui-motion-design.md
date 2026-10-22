# Ubi UI: expressive components and shared motion

Date: 2026-10-22
Status: proposed written design; awaiting user review before implementation planning.

## Intent and agreed direction

User requested many more Ubi components, including beautifully animated ones,
and research into Skiper and similar libraries before building. User approved
the researched direction with "yep exactly like those."

Combine Skiper's playful interaction details, Animate UI's everyday control
transitions, and selected React Bits / Magic UI effects. The
[reference report](../../ubi-ui-design-references.md) records nine libraries and
the proposed 30 additions. Build original, source-owned `.ubi` components for
the existing browser and Rust hosts. React remains documentation infrastructure.

Success means useful components that feel polished during actual interaction:
confident typography, deliberate spacing, responsive feedback, smooth motion,
working keyboard/touch behavior, and documentation that runs their real Ubi code.
Existing warm paper / forest ink / serif documentation identity stays coherent;
component demos may use richer accent, gradient and dark-surface treatments.

## Approaches considered

| Approach | Benefit | Cost |
| --- | --- | --- |
| Browser-only CSS effects | Fastest visual demo | Leaves the native renderer behind and splits the shared API |
| Shared typed motion plus staged components (selected) | One Ubi contract, testable behavior, reusable host machinery | Requires presentation state and scheduling in both hosts |
| General animation language / timeline engine | Maximum choreography flexibility | Larger language/runtime scope than this component expansion needs |

Use the second approach. No compiler syntax changes, animation framework
dependency, arbitrary CSS execution, model-driven frame loop, or AI judgments.

## Current implementation and change boundaries

The catalog has 40 components. `packages/ubi-ui/core.ubi` declares keyed
`View<M>`, `Node<M>`, `Kind`, `Prop<M>` and controlled `Event<M>` values.
`components.ubi` forwards per-component modules. Both hosts reject malformed
views; browser rendering commits model/view transactionally and retains keyed
DOM controls. Native core reconciles keyed trees; layout, input, painting and
platform scheduling are separate crates. Native scheduling currently uses
`ControlFlow::Wait`. There is no shared motion contract.

Changes belong in the Ubi package, Rust core/layout/input/render/platform crates,
browser validator/renderer/style files, examples, catalog and documentation.
Keep renderer-independent frame sampling in a focused Rust module and a matching
browser module; do not grow component-specific animation logic inside reducers.
No new crate is needed for the first slice.

Existing constructor signatures and imports stay available. Schema-1 views
without new properties remain valid. New properties/kinds are additive to the
current paired source/host release; old hosts reject unsupported additions rather
than silently accepting them. Expand both validators and their hostile-input
fixtures together. Retain current node, property, string and tree-depth budgets.

## Public motion contract

Add ordinary exported Ubi types to `core.ubi`:

```ubi
export enum Easing { Linear, EaseOut, EaseInOut, Spring }
export enum Transition { None, Fade, Rise, Scale }
export record Motion {
    durationMs: int,
    delayMs: int,
    easing: Easing
}
```

Add `Prop.Motion(Motion)`, `Prop.Enter(Transition)`, `Prop.Exit(Transition)`,
`Prop.Opacity(int)`, `Prop.Translate(int, int)` and `Prop.Scale(int)`.
Opacity and scale are percentages; translation uses logical pixels. Validate
duration `0..2000`, delay `0..1000`, opacity `0..100`, scale `25..200`, and each
translation coordinate `-100000..100000`. Reject malformed records, invalid enum
payloads, duplicate properties and out-of-range values in both hosts.

Generic motion interpolates opacity, translation and scale. Components can use
the same timing for named internal parts such as a switch thumb or tab indicator.
Generic width/height morphing is excluded; Accordion/Collapsible implement a
specific measured-height transition, with changing sibling layout accounted for.
Translation/scale apply around the node's center after layout. Ancestor transforms
compose; clipping, hit testing and focus geometry use the same presented geometry.

Normal containers do not animate without explicit motion. Existing controls get
documented defaults through component-kind resolution: quick feedback uses 140ms
EaseOut; selection indicators and switch thumbs use 220ms Spring; disclosure uses
220ms EaseInOut. Explicit zero duration disables motion, including ambient effects
on that component. Absence of `Motion` resolves to a component default; absence of
Enter/Exit resolves to None.

Enter/Exit reuse the resolved Motion timing. Fade changes opacity to/from zero;
Rise combines fade with a 12px downward offset; Scale combines fade with 96%
scale. Combine these with the node's declared opacity/translation/scale targets.
An explicit enter/exit on a general container uses 220ms EaseOut when no Motion
was supplied. All finite transitions finish at exactly the declared target.

For nonzero duration, let `t = clamp((now - start - delay) / duration, 0, 1)`.
Linear uses `t`; EaseOut uses `1 - (1-t)^3`; EaseInOut uses `4*t^3` below
`t=0.5`, otherwise `1 - (-2*t+2)^3/2`. Spring uses `s(t)/s(1)`, where
`s(t) = 1 - exp(-6*t) * (cos(10*t) + 0.6*sin(10*t))`. It is a fixed damped
curve with a finite deadline. Force exact endpoints; clamp presented opacity
and bounded control values to their legal ranges despite spring overshoot.
Zero duration applies targets immediately without delay. Rust/JavaScript fixture
samples must agree within `0.00001`. No physical spring configuration or
completion callbacks are public in this expansion.

## Frame, lifecycle and input behavior

Application models/reducers remain deterministic. Events commit logical state
immediately after successful validation. Hosts own monotonic clocks and per-key
presentation state; frames never call `update` or `view`. A failed application
commit leaves both logical state and existing animations unchanged.

On interruption, sample the current presented value and retarget from it. A
stable key and kind preserve animation identity. A changed kind starts a fresh
instance. Resizing, theme changes and scroll update geometry without resetting
application state. Resizing may recompute disclosure targets; it must not replay
entrances or retain stale hit bounds.

Removed nodes leave the logical tree, event dispatch, focus traversal and
accessibility tree immediately. If an exit is configured, retain a visual-only
snapshot until its deadline. It cannot accept input or keep browser IDs that
conflict with live controls. Reintroducing the same key/kind retargets from the
retained presentation; a different kind discards the old snapshot. Never retain
password/input values in detached exit snapshots; editables disappear immediately.

Permit at most 4096 live nodes and 4096 visual exit nodes per mount. Under exit
snapshot pressure, release oldest exits immediately. Disposal releases animations,
listeners, snapshots and timers. Disabled or closed content cannot be activated
during transitions; visual decoration cannot intercept input.

Browser frames use the host animation clock, scoped to the mount. Native hosts
request redraw only while finite motion is active, input changes, or an enabled
ambient effect is visible. Idle windows return to waiting. Hidden/occluded hosts
pause ambient work; finite transitions settle to their elapsed-time targets on
resume rather than replaying a backlog.

## Reduced motion and visual direction

Browser hosts observe `prefers-reduced-motion`, including changes while mounted.
Native hosts query the operating system preference through a platform adapter;
until available on a platform, use reduced motion conservatively. Expose a host
override for tests/embedders. User preference changes settle finite transitions
immediately and stop decorative loops. Components remain understandable without
movement; no essential content depends on a reveal, pointer proximity or autoplay.

Use short travel, restrained spring overshoot and purposeful hierarchy. Buttons
compress subtly on press; tabs glide between positions; disclosure content opens
without snapping; toasts enter cleanly and stack predictably. Rich cards use
shallow tilt and localized highlights. Animated text reveals once or on an explicit
replay. Ambient backgrounds are opt-in and subdued behind readable text. Focus
rings remain immediate, visible and unclipped throughout animation.

## Existing components to upgrade

| Component | Motion |
| --- | --- |
| Tabs | Sliding active indicator and short panel entrance; only active panel is interactive |
| SegmentedControl | Sliding selection surface; immediate selected semantics |
| Accordion / Collapsible | Measured expansion, clipping and chevron rotation; closed content leaves focus order immediately |
| Switch | Thumb travel, track color transition and subtle press feedback |
| Progress | Interpolated fill; announced value is the committed logical value |
| Skeleton | Subtle shimmer; static loading surface in reduced motion |

Accordion and Collapsible count separately as existing controls; these upgrades
add no catalog entries. Button hover/press feedback uses the same timing defaults.

## Thirty new components

All constructors live in separate `.ubi` modules with facade exports. Messages
remain typed; host-only animation never gains shell, file or network authority.

| Application component | Required behavior |
| --- | --- |
| Dialog | Controlled open state, labelled modal, focus containment, Escape/backdrop dismissal, focus return |
| AlertDialog | Explicit cancel/confirm; no backdrop dismissal; initial focus on safe action |
| Drawer | Controlled modal side panel; same focus/dismissal rules as Dialog; edge slide |
| Popover | Controlled anchored surface, viewport collision handling, Escape/outside dismissal, focus return |
| Tooltip | Hover/focus description; Escape closes; no focusable content; touch never requires it |
| Toast | Typed dismiss action, polite status announcement, optional timeout paused on hover/focus, visible close control |
| Select | Controlled single selection, arrows/Home/End/typeahead, selected value and disabled options |
| Combobox | Controlled query and selection; deterministic filtering in Ubi, keyboard list navigation and empty state |
| DropdownMenu | Triggered menu with arrows, typeahead, Escape, disabled items and focus return |
| ContextMenu | Pointer context request and Shift+F10; menu behavior shared with DropdownMenu |
| CommandPalette | Controlled searchable command dialog; deterministic filtering and selection; no global shortcut interception by default |
| Pagination | Controlled current page, bounded page count, previous/next disabled at boundaries |
| Stepper | Controlled current step and explicit navigation actions; no implicit form submission |
| Timeline | Ordered content with readable labels and progressive item entrances |
| Table | Semantic headers/rows/cells, controlled optional sorting; no virtualization, cell editing or server fetching |

| Expressive component | Required behavior |
| --- | --- |
| SpotlightCard | Pointer-local highlight; readable static appearance on keyboard/touch |
| TiltCard | Shallow bounded tilt and spring return; static reduced-motion/touch mode |
| CardStack | Controlled active item, previous/next actions, animated stack reordering |
| Carousel | Controlled current item, previous/next and keyboard actions; autoplay off by default |
| Dock | Labelled navigation actions with proximity scaling; stable accessible target area |
| Marquee | Decorative repeating content with pause control; single semantic copy; static reduced-motion mode |
| NumberTicker | Animated integer presentation with stable formatting; logical value announced once per update |
| TextReveal | Word-based entrance; continuous accessible text, explicit replay, static reduced-motion mode |
| RotatingText | Controlled current phrase; automatic rotation opt-in with pause and static preference fallback |
| ShimmerButton | Standard Button semantics plus optional sheen and busy state |
| BorderBeam | Decorative travelling border; no hit target or duplicate accessible content |
| AuroraBackground | Optional bounded animated gradient field; static reduced-motion appearance |
| GridBackground | Decorative grid with optional bounded highlight motion; static appearance available |
| ScrollReveal | One-shot entrance when visible within its scroll container; layout/content exist before reveal |
| AnimatedList | Stable keyed item entrances/exits/reordering; reading order follows committed list |

An application component is exported only when its required behavior works on
both hosts. Expressive components require the same semantic behavior and declared
static fallback on both hosts; renderer-specific visual enhancement is labelled
explicitly. Do not advertise shader, image loading or true 3D support through
decorative names. Carousel and CardStack accept existing child views, so they do
not depend on a new image pipeline. Date pickers, charts, drag/drop Kanban, arbitrary
timelines and a dynamic status island remain outside these 30 additions.

## Overlay foundations

Add `Kind.Overlay`, `Kind.MenuItem` and `Kind.OptionItem`. Add these Ubi types:

```ubi
export enum OverlayKind {
    Dialog, AlertDialog, Drawer, Popover, Tooltip, Toast,
    Select, Combobox, DropdownMenu, ContextMenu, CommandPalette
}
export enum Placement { Top, Right, Bottom, Left, Center }
```

Add `Prop.OverlayKind(OverlayKind)`, `Prop.Open(bool)`, `Prop.Anchor(string)`,
`Prop.Placement(Placement)`, `Prop.Dismiss(M)`, `Prop.TimeoutMs(int)`,
`Prop.LabelledBy(string)` and `Prop.DescribedBy(string)`. Dismiss dispatches
existing `Event.Activate(message)`; it does not introduce arbitrary callbacks.
Timeout is `0..60000` milliseconds, defaults to zero (no auto-dismiss), and is
valid only on Toast. Open defaults to false. OverlayKind is required on Overlay
and invalid on other kinds. Placement defaults to Bottom for anchored overlays,
Right for Drawer, and Center otherwise. Modal kinds are Dialog, AlertDialog,
Drawer and CommandPalette. Anchored kinds are Popover, Tooltip, Select, Combobox
and DropdownMenu, and require Anchor. ContextMenu uses its source trigger's
Anchor key, with the host remembering the context-request pointer position;
keyboard invocation anchors to trigger bounds. Toast uses the mount's upper-right
corner and stacks downward with 8px gaps. Closed overlays have no layout, input
or semantic participation; they may retain an exit snapshot.

MenuItem and OptionItem reuse Label, Content, Disabled, Option, Group, Selected
and Activate. Select/Combobox reuse existing typed Select events. Semantic label
references use keys, not raw browser IDs; reject dangling or self-referential
references. Every modal has Label or LabelledBy. Tooltip requires a description
relationship to its trigger. Extend validated role mappings for overlay, menu,
listbox and table semantics; do not accept arbitrary role strings.

Use one host-owned overlay
stack per mount for layering, anchoring, modal input isolation and focus restoration.
The active modal traps focus; content beneath it is inert. Escape dismisses only
the top eligible overlay. Outside dismissal requires both pointer-down and
pointer-up outside, preventing drag-release accidents. AlertDialog requires an
explicit action. Closing changes semantics immediately even while visuals exit.

Anchor references use validated keys in the same committed view. Reject dangling
anchors, invalid overlay relationships and cyclic dependencies. Position against
logical viewport bounds with 8px clearance; flip to the opposite side when the
preferred side cannot fit, then clamp. Reposition on scroll, resize and anchor
movement. Nested menus, multi-select and interactive tooltip content are deferred.
Browser and native fixtures cover the same kinds, property applicability,
relationships and dismissal messages before the overlay slice is exported.

## Delivery sequence

1. Shared motion contract, validators, pure frame sampling and both host schedulers.
   Prove interruption, reduced motion, rollback and idle cleanup first.
2. Upgrade existing controls and add NumberTicker, TextReveal, AnimatedList and
   ShimmerButton as the first visibly useful release.
3. Overlay foundations, Dialog, AlertDialog, Drawer, Popover, Tooltip and Toast.
4. Select, Combobox, menus, CommandPalette, Pagination, Stepper, Timeline and Table.
5. Remaining expressive components in small groups, ending with ambient effects.

Each slice includes Ubi examples, both hosts, catalog/docs updates and meaningful
commits. Failed acceptance blocks that slice's exports, not unrelated completed
work. Grow the catalog toward 70 only as real components become available.

## Verification and acceptance

- Compiler/interpreter/JavaScript agreement for every new source module and example.
- Matching Rust/JavaScript validation fixtures, including malformed motion and
  overlay values; preserve current bounds and transactional rollback tests.
- Deterministic clock samples at start, intermediate times, interruption and end;
  cross-host easing fixtures, exact final values and no reducer calls per frame.
- Input tests during transforms, animated disclosure and overlay entry/exit;
  keyboard navigation, modal focus, dismissal ordering and retained text selection.
- Real browser checks for all new mounts, representative interactions, theme
  changes, reduced-motion changes, 320/768/1440px layouts and CSS host isolation.
- Native sampling and GPU captures at known timestamps plus Windows window smoke
  for intermediate/final frames and idle scheduling. Still images alone cannot
  prove motion, focus behavior or lifecycle correctness.
- Real live documentation previews, highlighted copyable source, native evidence
  labelled separately, and honest per-component support notes.
- Relevant Rust/Node suites, formatting, Clippy, documentation build and diff checks.
  Record actual commands/results and unavailable environment checks at delivery.

Use `graphify-out/graph.json` as navigation evidence, verified against current
`View::decode`, `Tree::reconcile`, browser `mount`/`patch`, renderer `prepare`, and
native `Host::dispatch`/`refresh_scene`/window events. This design adds no runtime
behavior yet. iOS/Android device execution, Linux/macOS native execution and OS
screen-reader integration remain unclaimed until exercised.
