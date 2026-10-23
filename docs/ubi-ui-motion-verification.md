# Ubi UI motion and expansion verification

Verified 2026-10-23 against the approved [design](superpowers/specs/2026-10-22-ubi-ui-motion-design.md) and three linked [implementation plans](superpowers/plans/2026-10-22-ubi-ui-motion.md).

## User-reported rendering and spacing failures, 2026-10-23

The original completion claim overstated what the checks established. The browser harness imported the current development renderer, while the built documentation served older files from `previews/runtime`. A manual refresh had copied files to `previews/` instead. Passing reducer/mount checks did not detect missing visible content or collapsed effect layouts.

Reproduced before repair:

- NumberTicker and TextReveal had accessible labels but no visible text. Container reconciliation removed their renderer-owned spans. The same path erased table cells and tooltip content.
- Served Tooltip and ContextMenu could not dispatch opening requests because their runtime predated that repair.
- Compact/preview/control CSS ignored explicit source padding, gap and font-size values. Default inline zero padding also cancelled overlay CSS spacing.
- Effect surfaces, disclosure containers and overlays lacked the flex layout needed for child gaps. Card Stack collapsed to overlapping text; menu options appeared as unrelated primary buttons.
- Pointer tilt ignored duration zero and retained its tilted pose when reduced motion began.
- Search and password input placeholders were not rendered.

Repairs preserve renderer-owned content, honor source spacing, retain component CSS defaults, restore container layout and placeholders, give stacked cards opaque surfaces and navigation clearance, and settle disabled pointer motion. Exported previews and production output now undergo byte-for-byte checks against current runtime sources. Source interaction and catalog harnesses import that exported runtime; catalog checks require text/placeholder presence before and after updates.

Follow-up evidence: 56 Node tests passed; 13 real-browser renderer regressions and 12 exported-source interaction checks passed; the strengthened catalog matrix passed 71/71 at 320/768/1440px with light/dark and full/reduced motion. The final 82-page production build passed both runtime equality gates. Ticker, spotlight and stacked-card output were inspected visually, and live docs were refreshed. Rust/native code did not change in this follow-up. Exhaustive manual visual inspection of every component and physical mobile/native assistive-technology checks remain outside this evidence.

The initial rebuild encountered Windows `EBUSY` because the local HTTP server held `out/` as its working directory. Serving that directory from the repository root released the lock; rebuilding passed without disabling a check.

## Delivered

- 30 new source-owned Ubi components; 70 catalog entries total, preserving the original 40. All 71 applications (components plus gallery) compile and export.
- Shared bounded motion ABI and easing fixtures, interruption, visual-only exits, reduced-motion support and finite scheduling. Frames do not call application reducers or views.
- Application overlays, selection, menus, pagination, stepper, timeline and table; animated ticker, word reveal, keyed lists, cards, carousel, dock and decorative effects.
- Browser and native interaction infrastructure, including modal focus/dismissal, input composition, active-descendant combobox selection, tooltip/context requests and timer generation guards.
- Motion guide, all 70 component pages, live compiled-source previews and 140 native light/dark captures.

## Original integration checks (historical)

| Check | Result |
| --- | --- |
| `rtk cargo test --locked --workspace --features native-ui -j 1` | 336 passed across 40 suites, including doctests; 230.15 seconds |
| `rtk cargo fmt --all -- --check` | Passed |
| `rtk cargo clippy --locked --workspace --all-targets --features native-ui -- -D warnings` | Exit 0; no Clippy errors; Cargo emitted filesystem hard-link fallback notices |
| `rtk proxy npm test` in `ubi-ui/docs` | 54 Node tests passed |
| `rtk proxy npm exec -- next build --webpack` in `ubi-ui/docs` | 82 static pages built |
| Real browser catalog harness | 71/71 applications; 320/768/1440px, light/dark, full/reduced motion |
| Real browser renderer harness | 11/11 passed: rollback, focus/caret, IME, independent mounts, finite scheduling, reduced motion, disposal and private exit snapshots |
| Real compiled-source interaction harness | 9/9 passed: Dialog, AlertDialog, Select, Combobox, Toast, Tooltip, ContextMenu, Carousel and NumberTicker |
| Real browser host-request/motion harness | 4/4 passed, including disabled-anchor rejection and centered modal transform interpolation |
| Final native window smoke | `UBI_NATIVE_UI_SMOKE_OK`; Carousel, 10 nodes, NVIDIA GeForce RTX 4070 Ti SUPER |
| Native component captures | 140 PNGs from 70 checked source examples, both themes; AMD Radeon(TM) Graphics |
| Native motion captures | Five real source apps at 0/110/440ms; all five midpoint and endpoint images differ visibly from the initial image |
| `rtk git diff --check` | Passed |

Shared tests consume 17 motion samples, 14 overlay contracts and 12 effect contracts. Compiled-source native/JavaScript parity and native source interaction tests are included in the workspace suite. The module budget is now 128 to accommodate the source library; the 4MiB aggregate source budget, token budget and depth limit remain unchanged. The boundary regression accepts 128 and rejects 129 modules.

## Reproduction and artifacts

- Browser harnesses: `ubi-ui/tools/browser_catalog.html`, `ubi-ui/tools/browser_interactions.html`, `ubi-ui/web/tests/browser.html` and `ubi-ui/web/tests/renderer-open-request.html`. Serve the repository on loopback and visit these pages.
- Export checked applications with `cargo run --locked --example ui-web-export -- ubi-ui/docs/public/previews`. The exporter copies the complete runtime allowlist and canonical codec.
- Build docs with `npm --prefix ubi-ui/docs run build`; its prebuild regenerates examples. The final verification used the direct Next command after the unchanged 71 source applications were exported and final runtime modules refreshed.
- Generate native component images with `native-ui-capture --components <existing-empty-directory>` and motion samples with `native-ui-capture --motion <existing-empty-directory>` after building the example with `--features native-ui`.
- Final captures are installed in `ubi-ui/docs/public/components`. Local raw motion evidence is in `.ubi-build/motion-verified-samples`; timestamps use `CaptureSession::write_at` through the real source application. Pixel comparison confirmed visible changes for Switch, NumberTicker, TextReveal, AnimatedList and Carousel. This demonstrates sampled animation, not a frame-rate benchmark.

## Review corrections

Review and real captures exposed and fixed stale timer callbacks, initial-mount list jumps, inactive carousel semantics, lost combobox focus/caret, hidden host-request rejection, blank native tooltip/word rendering, snapshot event payload retention, failed-patch timeline restart, stale context-menu pointer placement and overlay positioning overriding animated transforms. Text reveals settle when duration becomes zero without changing logical text; progress and tickers keep committed semantic values while presentation animates.

The new context-menu browser check initially used an incorrect item key; corrected to the actual source constructor key. The new modal-motion test initially supplied a normalized easing string to the raw tagged ABI; corrected its fixture and frame-clock advancement. Both pass on the final runtime.

## Environment and coverage limits

The first full run passed its executable tests but rustdoc failed against concurrently rebuilt artifacts. After freezing code, another run hit Windows linker LNK1104 opening an existing dependency file. The final serialized run passed all 336 tests and doctests. No compiler setting or test was disabled to obtain that result.

Windows denied a generated `.next` directory in the sandbox; the production build passed with required filesystem access. A temporary account usage limit also blocked browser observation; account access subsequently recovered and all final browser harnesses above were observed passing.

Native coverage is Windows desktop with logical input/accessibility tests, GPU captures and a real window smoke. Physical Android/iOS devices, other native operating systems, OS screen readers and manual native keyboard/IME automation were not verified. Browser tilt/proximity/ambient decoration may use documented static native treatments. No continuous-frame-rate or universal platform-readiness claim is made.

## Execution decisions

The user approved all three plans and execution, and restricted subagents to GPT-6 Luna at max reasoning. All implementer/reviewer subagents used that configuration. Work reused the current feature checkout. Intermediate 44/59/etc catalog checkpoints were consolidated into the final 70-component verification; test names and commit boundaries were adapted to shared host contracts. Sortable headers reuse Change plus Option; tooltip descriptions derive from validated anchors. React remains documentation infrastructure, not the component implementation.
