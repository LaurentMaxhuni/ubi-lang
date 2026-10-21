# Ubi UI foundation verification — 2026-10-21

Implemented in `examples/ui/`: reusable Ubi text, headings, column/row layouts,
card, badge, button, theme/pressed helpers, typed views/events, an Ubi-authored
explorer and web/desktop/mobile packages. Remaining primitives, forms, overlays,
blocks, installer and framework adapters are later work.

## Executed checks

| Check | Result |
| --- | --- |
| Independent Ubi contracts | 9 groups pass on interpreter and JavaScript backend with fixed expected views/messages/state |
| Codec/graph validation | 9 Node tests pass; exact byte/node/key/depth boundaries, Unicode, floats, malformed graphs/props, bounded shared-DAG expansion |
| Package integration | 2 Rust tests pass for shared modules, public ABI, relative offline assets and safe hosts |
| Full Rust regression | 233 tests pass in 10 suites, including existing corpus and mutation checks |
| Static checks | Rust/Ubi formatting, Clippy `--all-targets -- -D warnings`, Git whitespace checks pass |
| Actual browser harness | 12 checks pass: scoped CSS, keyed identity/focus, disabled/busy controls, mount isolation/stale events, failures, rollback/recovery, reorder/replacement, reentry, theme cleanup |
| Native keyboard | Enter then Space on browser probe produce count 1 then 2; pointer/Enter also work through compiled explorer exports |
| Responsive matrix | Both sections and Light/Dark at 320/768/1440 CSS px: no horizontal document overflow or host fault; every visible button at least 44 CSS px high |
| Windows executable | Actual packaged `UbiUI.exe --smoke-test` exits 0: Ubi transitions, theme, composition, reset, no Node globals and forbidden asset returns 403 |
| Android | Debug APK builds, installs and launches; actual WebView smoke passes across cold starts: initial state, transitions, theme, composition, reset and no overflow |

Run the browser harness with `node examples/ui/tests/serve.mjs`; open its printed
URL. Run the explorer with `ubi dev --root examples/ui --port 8792`.

## Rendered theme contrast

Ratios measured from computed CSS properties; these specific pairs do not
establish full WCAG conformance. Disabled controls are excluded.

| Pair | Light | Dark |
| --- | ---: | ---: |
| Reading text / surface | 15.81 | 14.31 |
| Muted text / surface | 5.59 | 7.53 |
| Primary foreground / primary background | 5.39 | 7.07 |
| Primary text / soft background | 4.68 | 5.06 |
| Destructive foreground / background | 6.49 | 7.75 |
| Success text / soft background | 5.74 | 6.98 |
| Warning text / soft background | 5.85 | 7.48 |
| Control border / surface | 3.77 | 5.05 |

## Actual platforms

Host: Windows 11 Pro 10.0.26300, Node 22.22.1. Browser checks used Chrome 155.
The packaged Windows executable uses pinned Electron 44.7.0/Packager 20.3.0 and
reports Chromium 152.0.7977.130. It runs through the actual `ubi://app/` handler.

Android: isolated read-only `Medium_Phone` emulator `emulator-5582`, Android 17,
WebView `com.google.android.webview` 145.0.7632.218. The existing project-local
Temurin 21 JDK, Android SDK and Gradle cache built the APK (123 tasks). SDK
downloads were disabled; the existing verification init script selects installed
API 36.1 without changing committed platform defaults. No saved AVD snapshots
are loaded or written; the test emulator is stopped after verification.

The normal generated APK was built/installed/launched first. A subsequent
verification-only MainActivity hook in ignored generated output invoked DOM
buttons inside the actual WebView and logged `UBI_UI_ANDROID_SMOKE_OK`. The hook
is not shipped in templates and does not establish physical touch or assistive
technology coverage. Cold starts assert count 0 before interacting; this pure
UI example has no persistence. Existing mobile dependencies were copied into
an isolated verification directory after an offline npm attempt found an uncached
package. No new product dependency versions were introduced.

Logs/screenshots remain under ignored `.ubi-build/ui-foundation/`: full-tests,
desktop-package/smoke, android-build/smoke-build/smoke logs, Android screenshot
and explorer screenshots. Historical task-app platform checks are not UI checks.

## Independent review and limits

Final review used GPT 6 Luna at Max. It found global stylesheet resets leaking
into host pages. A regression first failed on the host page margin, then passed
after library rules were scoped under `data-ubi-mount`. Explorer page reset/error
styles are now in separate `shell.css`, explicitly allowed by Electron. Existing
focus/rollback/lifecycle/package checks remain green.

- iOS generation/build/runtime is unverified: no macOS/Xcode host used here.
- 200/400% native browser zoom, screen readers, OS forced-colors/reduced-motion
  settings, physical touch, virtual keyboards and rotation remain unverified.
  IAB's zoom shortcut did not change viewport/pixel ratio; 320px reflow is recorded
  separately. CSS provides focus, forced-color, reduced-motion and safe-area
  rules, whose presence alone does not prove audit coverage. Forms/IME checks
  belong to the next control wave.
- Linux/macOS desktop and other browser engines were not launched for this
  slice. No store signing, publishing or framework adapters were attempted.
- Package-test temp-file replacement was denied inside the Windows sandbox;
  package tests/full suite passed elevated. Rust used its incremental hard-link
  fallback. Existing Windows symlink privilege skips remain skips. Hidden
  conformance remains evaluator-owned.

Execution decisions: used a clean feature branch in the existing checkout to
retain platform tooling (less isolation than another worktree), and a PowerShell
progress ledger in the ignored task directory (manual bookkeeping). The existing
Graphify index did not cover new UI modules, so changed sources/imports were
verified directly. These decisions introduce no runtime feature variation.
