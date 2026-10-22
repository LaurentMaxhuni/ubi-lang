# Native Ubi UI verification — 2026-10-22

Local environment: Windows, Rust 1.97.1, Node 22.22.1, npm 11.19.0.

## Executed checks

| Check | Result |
| --- | --- |
| `cargo test --workspace --features native-ui --locked` | 252 tests pass across 28 suites, including existing compiler/JavaScript/package tests |
| `cargo clippy --workspace --all-targets --features native-ui -- -D warnings` | Pass; no source diagnostics; sandbox incremental-cache hardlink warnings are environmental |
| `cargo fmt --all -- --check` and default-feature Clippy | Pass |
| Native GPU readback | Rounded button, fill, text and glyph-cache reuse pass; no silent GPU skips |
| Native window smoke | `UBI_NATIVE_UI_SMOKE_OK adapter=NVIDIA GeForce RTX 4070 Ti SUPER nodes=49` after actual window creation, surface acquisition, rendering and presentation |
| Native captures | Actual Ubi gallery rendered on AMD Radeon(TM) Graphics at 1100×1000 light/dark and 420×1000 narrow |
| Ubi interaction | Keyboard input → typed event → Ubi reducer → changed counter node; typed text/toggle/slider/theme events and equal-state no-op tested |
| Transaction failure | Invalid view and invalid event payload preserve last committed model/tree/revision |
| Lifecycle regressions | Queued pointer input after unmount and replacing container/control kinds fail before fixes and pass afterward |
| Layout | Grid, percentages, min/max, absolute offsets, cached resize, clipped scrolling, focus reveal, resource cleanup, 420/640/1100px control containment pass |
| Documentation production build | Next.js 16.4.2 / React 19.3.1 / Tailwind 4.3.3 static export passes using webpack |
| Documentation browser | Navigation, loaded native PNGs, radio-based dark capture selection and no horizontal page overflow at 750px verified |
| Independent review | Native ABI, state transactions, input/capture, layout and renderer reviewed; both identified lifecycle issues reproduced and corrected |

Sandboxed full tests initially hit the existing dev-server socket restriction
(Windows 10013). Running the same suite with the required local process/socket
access passes. Cargo's sandbox cannot hardlink its incremental caches and copies
files instead; that does not represent a source warning. Next's Turbopack worker
exits prematurely both inside and outside the sandbox. The supported webpack
build path passes and is selected in the portal scripts.

## Benchmark

`cargo bench --locked --features native-ui --bench native_ui` runs a release-mode
49-node Ubi gallery, with 20 warmups and 500 measured real model/event/view calls.
Every counter event must update exactly the `count` node. Another 500 reset calls
assert that equal state skips the view function.

Final measured result on this host:

- Checked native program: 5.41 ms.
- Ubi update + complete derived view + validation/reconciliation: p50 2.88 ms,
  p95 4.13 ms.
- Equal-state reducer: 16.7 µs/call; zero additional view evaluations.

These are CPU wall-clock measurements on one machine, not a frame-rate promise
or GPU timing. No unstable timing thresholds are asserted by tests.

## Honest scope

Windows native execution is verified. Native Linux/macOS execution, mobile and
WebAssembly hosts, physical-device testing and OS screen-reader bridges have not
been verified or delivered. Accessibility snapshots are groundwork, not proof of
assistive-technology compatibility. Advanced shaping, fallback fonts, bidi,
clipboard, text-range selection, horizontal scrolling and animated transitions
are absent. Arbitrary custom tokens can be supplied by Rust renderer embedders;
the Ubi package currently selects named themes and component props.

State and views are immutable Ubi values with transactional host ownership.
Changed models still evaluate the full Ubi view function; fine-grained automatic
signal dependency tracking is not claimed. The retained tree and layout caches
prevent remounting or relaying out unchanged components. Pure reducer/view work
is synchronous and bounded; large workloads need a future worker protocol.

Radio/segmented/toggle groups, multiline/numeric/select/combo/search inputs,
split views/tabs, navigation/overlays and the remaining feedback components are
deferred. Only working constructors appear in the source package. Future motion,
effects, blocks, data, AI and icon packages are not created during Phase 1.

Repository graph was refreshed locally with `graphify update . --no-cluster`:
1,803 nodes / 4,929 edges, deterministic code extraction with no model calls.
Generated `graphify-out/graph.json` remains ignored.
