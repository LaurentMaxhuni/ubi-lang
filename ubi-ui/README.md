# Ubi UI — native core

Original Rust component runtime with Ubi source APIs. This desktop backend runs
the existing checked Rust interpreter directly; it needs no Node, DOM or
JavaScript host. Existing Ubi JavaScript, Electron and Capacitor targets remain
available independently.

## Run

From the repository root, with Rust/Cargo and a desktop GPU driver:

```powershell
cargo run --locked --features native-ui -- ui examples/gallery.ubi --root ubi-ui
cargo run --locked --features native-ui -- ui examples/counter.ubi --root ubi-ui
```

Tab/Shift+Tab move focus and reveal offscreen controls. Enter/Space activate
buttons and toggles. Slider supports drag, arrows, Home/End. Inputs support
Unicode scalar editing, Home/End, Backspace/Delete, select all and IME commit.
Pointer release activates only the captured control under the pointer. Disabled
and busy controls neither focus nor dispatch. Escape/window focus loss cancel
capture. ScrollView clips both drawing and hit testing; nested wheel routing
falls back to an ancestor at a scroll boundary.

## Write Ubi

Import ordinary source files from `packages/ubi-ui/`. No registry or special UI
syntax is assumed. Export three concrete, pure synchronous functions:

```ubi
export fn init() -> Model { /* initial model */ }
export fn update(model: Model, event: Event<Message>) -> Model { /* new model */ }
export fn view(model: Model) -> View<Message> { /* keyed component tree */ }
```

See the complete [counter](examples/counter.ubi) and [gallery](examples/gallery.ubi).
`core.ubi` defines schema-1 records/tagged enums; `components.ubi` provides the
implemented component constructors. Containers take `(key, props, children)`.
Controls take their content/value/message followed by optional props. Keys are
globally unique per view. Duplicate props are rejected; do not pass a prop already
owned by a constructor (for example Content on a Button or Width on IconButton).

The native host owns model state, validates event bindings and Ubi types, and
commits model/tree together after successful evaluation and view validation.
Equal models skip `view`. Reconciliation retains matching keys and kinds, reports
mount/update/unmount deltas and releases focus/scroll/layout resources. Replacing
a kind remounts its key. Derived state uses normal pure Ubi functions. Automatic
dependency-tracked signals are not implemented; changed models evaluate the full
Ubi `view`, then only changed keyed nodes invalidate layout.

## Crates

| Crate | Responsibility |
| --- | --- |
| `ubi-ui-core` | Validated trees, component properties, lifecycle changes, accessibility records |
| `ubi-ui-layout` | Retained Taffy nodes, flex/grid/absolute layout, measurement, clipping, scroll geometry |
| `ubi-ui-input` | Focus, hit testing, capture, keyboard controls, editing, typed events |
| `ubi-ui-theme` | Light/dark/custom Rust tokens and subtree inheritance |
| `ubi-ui-render` | Original batched wgpu 2D pipeline, rounded primitives, cached fontdue glyph atlas |
| `ubi-ui-platform` | winit desktop lifecycle, surfaces, DPI, event routing and PNG capture |

The compiler library adds `Compiler::runtime()` and `Runtime::invoke()` using
owned JSON data, recursively validated against actual Ubi types. It promotes the
existing interpreter; AST/type/effect/module semantics and JavaScript generation
are reused. Native exports cannot receive functions or implicit host authority.
The existing one-million-step, 32-call-frame and 32-aggregate-depth limits apply.
Native boundary limits: 100,000 values, 4 MiB string/key bytes. UI limits: 4,096
nodes, depth 32, 256-byte keys, 64 props/node and 16 KiB per text field.

Layout uses logical pixels and cached intrinsic text measurements; glyphs render
at physical DPI. One atlas binding and one draw call render each submitted frame.
Idle windows wait for events; unchanged hover positions do not request frames.
Glyph/measurement caches and per-frame vertex counts are bounded. Atlas recycling
does not invalidate a frame already in flight because queue writes/submissions
retain their order. There is no claim of GPU performance based on CPU timings.

## Implemented components

Window, View, Column/Row/Flex, Grid, ScrollView, Card, Separator, Text/Heading,
Button, IconButton, Checkbox, Switch, Slider, TextInput, PasswordInput, Badge and
ProgressBar. Each interactive family has enabled/disabled/focus/hover/pressed
handling as appropriate. Inputs are single-line and controlled by Ubi state.

RadioGroup, SegmentedControl, ToggleGroup, TextArea, NumberInput, Select, ComboBox,
SearchInput, SplitView, Tabs, navigation/overlays, Toast, Spinner, Skeleton and
Avatar are deferred, with no placeholder exports. Motion/effects/blocks/data/AI/
icons packages are not implemented during this phase.

## Documentation and captures

```powershell
cd ubi-ui/docs
npm ci
npm run dev
# Production static export to out/:
npm run build
```

Next.js/React/Tailwind are confined to this documentation portal. It reads actual
Ubi declarations and the counter example during build. Gallery tabs switch
verified images; they do not emulate native controls in React.

Generate new captures in an empty directory (files never overwrite):

```powershell
cargo run --locked --features native-ui --example native-ui-capture -- <existing-empty-directory>
cargo run --locked --features native-ui -- ui examples/gallery.ubi --root ubi-ui --capture <new.png>
cargo run --locked --features native-ui -- ui examples/gallery.ubi --root ubi-ui --smoke
```

`--smoke` creates a hidden native window, acquires a swapchain frame, renders the
Ubi view and presents it, then exits with a success marker. It does not claim
real OS keyboard automation. Separate Rust tests execute input-to-Ubi event flow.
Bundled Inter and its OFL license are in `assets/` and the portal's public assets.

## Verification and limitations

```powershell
cargo test --locked --workspace --features native-ui
cargo clippy --workspace --all-targets --features native-ui -- -D warnings
cargo fmt --all -- --check
cargo bench --locked --features native-ui --bench native_ui
```

See [verification evidence](docs/verification.md) for executed checks, measurements
and platform status. GPU tests require a working adapter and fail explicitly when
one is unavailable. In headless CI, configure a supported software adapter rather
than silently skipping the renderer tests.

Windows is verified. Linux/macOS native execution, WebAssembly/mobile hosts,
native screen-reader bridges, advanced text shaping/bidirectional text/font
fallback, clipboard and range selection, horizontal scrolling, arbitrary Ubi
color-token definitions and animated transitions are not implemented/verified.
Accessibility records expose roles/names/values/states/bounds; password values
are redacted. Pure reducers run on the event thread under execution limits, so
expensive work belongs outside update/view. Events failing evaluation currently
report to stderr and preserve model/tree. An application error overlay and async
worker protocol remain future work.
