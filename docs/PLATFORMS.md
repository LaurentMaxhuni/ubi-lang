# Platform readiness

Scope: shared Ubi applications on web, Node CLI, desktop, and Android/iOS mobile.
"All versions" is interpreted as these platform targets, with supported runtime
floors rather than arbitrary historical browser or OS versions.

## Local prerequisite inventory, 2026-10-09

| Requirement | Available evidence | Status / owner |
| --- | --- | --- |
| Rust compiler and Cargo | Rust/Cargo 1.97.1 installed | Ready; agent |
| Node and npm | Node 22.22.1, npm 11.19.0 installed | Ready; agent |
| Browser web host | Dev and packaged static bundle checked in Codex browser | Passed; agent |
| Windows desktop build/launch | Actual packaged UbiTasks.exe passes persistence/isolation smoke | Passed; agent |
| Android SDK | SDK root at `%LOCALAPPDATA%/Android/Sdk`; platforms 36.1/37.0, build-tools 36.0.0/36.1.0/37.0.0 | Ready; agent |
| Android emulator | Emulator 37.1.11; Medium_Phone and Pixel_10_Pro_XL AVDs | Ready; agent |
| Java | Project-local checksum-verified Temurin 21.0.12.1+1 builds Android | Passed; agent |
| Android debug signing | Generated debug signing, no developer account required | Local route only; agent |
| macOS/Xcode build host | No macOS execution host available in this session | blocked-on-human; user supplies host or CI execution |
| Linux desktop build/launch | No Linux execution host available in this session | blocked-on-human; user supplies host or CI execution |
| iOS simulator/device | Requires macOS/Xcode and simulator/device | blocked-on-human; user supplies host |
| Store distribution/signing | No signing credentials requested or inspected | Not required for local debug/portable packages; required before store release |
| Hidden conformance | Evaluator-owned corpus unavailable | blocked-on-human; independent evaluator |
| GitHub CI execution | `gh auth status` reports no signed-in hosts | blocked-on-human; user signs in before dispatch |

SDK licenses already installed are reused. No global Java/Android settings,
signing identities, or developer accounts are changed. Test emulators use
temporary read-only images and do not save snapshots to existing AVDs.

## Support contract

Application build tools require Node 22.12 or newer. Native dependencies are
pinned in the application lockfile. Web requires ES modules, localStorage,
AbortController, and Unicode-capable current browsers; automated browser
evidence establishes the actually checked versions.

Electron desktop and Capacitor mobile use the same compiled task domain;
native prerequisites come from [Electron platform support](https://github.com/electron/electron#platform-support)
and [Capacitor environment setup](https://capacitorjs.com/docs/getting-started/environment-setup).
Capacitor 8 requires Node 22+, Xcode 26+ for iOS, and Android SDK/Android Studio.
Actual SDK targets and device floors come from the pinned generated projects.

## Executed application evidence

Compiler baseline: 151 tests pass outside the Windows sandbox. Inside the
sandbox, child compiler processes cannot canonicalize temporary project roots;
this is an environmental restriction rather than a recorded compiler failure.

V1 verification on this machine:

| Target/check | Observed outcome |
| --- | --- |
| Compiler/conformance | 178 tests pass; formatting and Clippy with denied warnings pass |
| Mutation watchdog | 10,000 parser/checker mutations and expanding-instantiation checks pass under separate 55-second process watchdog |
| Web development | Keyboard create, complete, filter, reload persistence; narrow layout has no horizontal overflow |
| Packaged web | Static HTTP bundle loads relative compiled imports; keyboard create/complete survives reload |
| CLI | Fresh package creates/completes/lists a persisted task using a task-owned store; independent corruption/failed-write checks pass |
| Windows desktop | Electron 44.7.0 / Packager 20.3.0 produce `UbiTasks-win32-x64/UbiTasks.exe`; actual executable smoke exits 0 with `UBI_DESKTOP_SMOKE_OK`, verifies sandbox and persistence |
| Android | Capacitor 8.5.3 / Preferences 8.0.1 debug APK builds, installs, creates/completes tasks; completion survives background/resume and force-stop/relaunch on API37 emulator |
| Formatter/project creation | Independent token/comment/idempotence and no-overwrite/invalid-input/read-only checks pass; replacement rollback reviewed; fresh init checks/packages for all four targets and is canonical |
| Artifact data preservation | Independent red-to-green hardlink regressions verify build/package never mutate unrelated linked file bytes |
| Windows symlink cases | Unchecked: creation denied with Windows privilege error 1314; cross-platform CI includes these tests where supported |
| Other native targets | macOS/Linux/iOS jobs configured; not executed in this session |

See [Android evidence](android-verification.md) for prerequisites, local SDK
override, Gradle cache recovery, and device commands. `node scripts/verify-desktop.mjs`
launches the actual packaged desktop executable and checks its smoke result.

CI runs compiler tests on Windows/Linux/macOS and Node22/24, builds/launches
desktop packages, builds Android debug APKs, and builds unsigned iOS simulator
apps. Workflow configuration is not evidence of executed checks. Release 1.0,
store distribution/signing, physical devices, older runtime floors, and hidden
conformance remain pending; no unavailable check is counted as passed.
