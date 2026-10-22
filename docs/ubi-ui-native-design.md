# Ubi UI Phase 1: native foundation

## Repository findings

Ubi is a Rust 2021 compiler with a filesystem-independent `Compiler`, bounded
source loading, a lexer/parser, nominal types, generics, checked effects, an IR,
and a JavaScript ES-module backend. `ubi run` executes generated code in Node.
Desktop/mobile packages currently embed browser hosts (Electron/Capacitor).
Those hosts are independent of this native backend.

The existing Rust interpreter evaluates the checked AST, uses immutable
reference-counted aggregates and snapshot closures, validates exported calls,
and limits execution to one million steps, 32 call frames and 32 aggregate
levels. It currently compiles only for tests. Ubi has relative source imports,
records, tagged enums, immutable lists, pure functions and reducers, but no
native FFI, UI syntax, tracing garbage collector or package registry.

`examples/ui/src/ui` already defines a flat, keyed `View<M>` schema and generic
component functions. Reuse its representation and conventions. Do not change
the existing browser renderer or invent new syntax. Promote the interpreter
behind a small public, typed JSON invocation boundary. Native exports are pure
and synchronous; no implicit host capabilities.

## Native architecture

Cargo workspace: existing compiler plus `ubi-ui/crates/ubi-ui-{core,layout,
theme,input,render,platform}`. Core owns validated keyed trees, reconciliation,
lifecycle deltas and accessibility snapshots. Layout owns retained Taffy nodes,
intrinsic measurement, flex/grid/absolute sizing and scroll/clipping geometry.
Theme owns inherited semantic tokens. Input owns hit testing, focus, pointer
capture, keyboard activation, editing and controlled events. Render owns text
measurement/raster caching and a batched wgpu 2D pipeline. Platform owns the winit
event loop and GPU surface lifecycle. Compiler changes stay confined to native
execution and an opt-in `ubi ui` command.

Use winit 0.30, wgpu 30, Taffy 0.15, fontdue 0.9. These are infrastructure, not
component toolkits. GPU triangles render rounded rectangles and cached glyphs;
logical pixels become physical pixels once at the rendering boundary. Use a
bundled, licensed font so installation does not depend on OS font paths.

## Ubi API and state

`ubi-ui/packages/ubi-ui/core.ubi` and `components.ubi` supply ordinary Ubi source
modules. Applications export `init() -> Model`, `update(Model, Event<Message>)
-> Model`, and `view(Model) -> View<Message>`. The native host owns the model,
serializes typed events, and commits model and tree together only after update,
view and validation succeed. Equal state skips view evaluation. Keyed tree
reconciliation preserves focus, scroll and layout nodes, reports mount/update/
unmount, and invalidates only changed geometry. Derived state is ordinary pure
Ubi functions. Evaluating `view` after a changed model remains whole-view work;
automatic dependency tracking is outside this foundation.

Essential native components: Window/View, column/row/flex, grid, ScrollView,
Card, Text/Heading, Separator, Button/IconButton, Checkbox, Switch, Slider,
TextInput/PasswordInput and Badge/ProgressBar. Controls are controlled by the Ubi
model, include disabled/focus/hover/pressed states and keyboard operation.
Components without working behavior are not exported. Other requested families
remain follow-up work, as permitted by the brief's correctness-first priority.

Accessibility snapshots expose stable keys, roles, names, values, bounds,
disabled/checked/focused states and children. Native screen-reader adapters are
an explicit remaining limitation, not a claimed integration.

## Visual language

Quiet graphite surfaces, cobalt accent, generous 8px spacing, 40px controls,
clear typographic hierarchy, restrained 8/12px corners, fine borders and visible
keyboard focus. Light and dark share semantic tokens; per-subtree theme
overrides inherit. Hover/press feedback is immediate; decorative motion is
excluded until a reduced-motion-aware animation system exists.

References studied for visual/interaction guidance only:
[Aceternity](https://ui.aceternity.com/components),
[Untitled UI buttons](https://www.untitledui.com/components/buttons).
No reference source is copied. Runtime design follows official
[wgpu](https://docs.rs/wgpu/30.0.1/wgpu/),
[winit](https://docs.rs/winit/0.30.13/winit/), and
[Taffy](https://docs.rs/taffy/0.15.0/taffy/) APIs.

## Verification and delivery

Test native ABI validation/budgets, hostile trees, keyed lifecycle, no-op updates,
transaction rollback, focus/capture cleanup, Unicode editing, disabled controls,
resizing, DPI, layout constraints, clipping, scroll bounds, theme inheritance,
text caching, GPU offscreen readback and a real Ubi model/event/view round trip.
Provide a native Ubi gallery, executable benchmark and separate Next/React/
Tailwind documentation portal. Portal examples use actual Ubi sources and
native renderer captures, never React recreations of controls.

Windows is the initial executed desktop target. Record actual checks and any
unavailable GPU/window/platform checks. WebAssembly and mobile are architectural
extension points; no native backend support is claimed without execution.
