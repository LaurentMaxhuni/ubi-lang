# Platform readiness

Scope: shared Ubi applications on web, Node CLI, desktop, and Android/iOS mobile.
"All versions" is interpreted as these platform targets, with supported runtime
floors rather than arbitrary historical browser or OS versions.

## Local and hosted prerequisite inventory, updated 2026-10-14

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
| macOS/Xcode build host | Hosted macOS 26.6.2 / Xcode 26.6 builds desktop and unsigned iOS simulator apps | Passed; hosted CI |
| Linux execution host | Existing Ubuntu 24.04.4 WSL2 x86_64 with WSLg; project-local Rust 1.99.0, Node 22.22.1/24.21.0 and extracted compiler/runtime libraries | Passed; independent full suite, CLI and packaged desktop |
| iOS simulator/device | Unsigned simulator application builds on hosted macOS; UI/lifecycle and device checks not executed | Build passed; runtime verification pending |
| Store distribution/signing | No signing credentials requested or inspected | Not required for local debug/portable packages; required before store release |
| Hidden conformance | Evaluator-owned corpus unavailable | blocked-on-human; independent evaluator |
| GitHub CI execution | All 11 compiler/desktop/mobile jobs pass for a6328bc | Passed; hosted CI |

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

Historical compiler baseline: 151 tests passed outside the Windows sandbox. Inside the
sandbox, child compiler processes cannot canonicalize temporary project roots;
this is an environmental restriction rather than a recorded compiler failure.

Local and hosted V1 verification:

| Target/check | Observed outcome |
| --- | --- |
| Compiler/conformance | 180 tests, formatting and denied-warning Clippy pass on hosted Windows/Linux/macOS with Node22/24; 180 tests also pass locally on Windows; earlier independent Linux evaluation passed178 |
| Mutation watchdog | 10,000 parser/checker mutations and expanding-instantiation checks pass under separate 55-second process watchdog |
| Web development | Keyboard create, complete, filter, reload persistence; narrow layout has no horizontal overflow |
| Packaged web | Static HTTP bundle loads relative compiled imports; keyboard create/complete survives reload |
| CLI | Fresh package creates/completes/lists a persisted task using a task-owned store; independent corruption/failed-write checks pass |
| Windows desktop | Electron 44.7.0 / Packager 20.3.0 produce `UbiTasks-win32-x64/UbiTasks.exe`; actual executable smoke exits 0 with `UBI_DESKTOP_SMOKE_OK`, verifies sandbox and persistence |
| Linux CLI | Node 22.22.1 on Ubuntu 24.04.4 WSL2 creates/completes/lists a persisted task |
| Linux desktop | Electron 44.7.0 / Packager 20.3.0 produce `UbiTasks-linux-x64/UbiTasks`; source and actual packaged executable smoke both exit 0 with `UBI_DESKTOP_SMOKE_OK` under WSLg, with sandbox enabled |
| Android | Capacitor 8.5.3 / Preferences 8.0.1 debug APK builds, installs, creates/completes tasks; completion survives background/resume and force-stop/relaunch on API37 emulator |
| Formatter/project creation | Independent token/comment/idempotence and no-overwrite/invalid-input/read-only checks pass; replacement rollback reviewed; fresh init checks/packages for all four targets and is canonical |
| Artifact data preservation | Independent red-to-green hardlink regressions verify build/package never mutate unrelated linked file bytes |
| Accepted socket timeouts | Independent forced-nonblocking regression fails before blocking-mode fix and passes afterward; idle requests still return 408; hosted macOS Node22/24 dev checks pass |
| Windows symlink cases | Unchecked: creation denied with Windows privilege error 1314; cross-platform CI includes these tests where supported |
| Linux symlink cases | Independent file/directory fixture prerequisites and CLI/dev/package containment checks execute and pass without skipped fixtures |
| macOS desktop | Hosted native source smoke, packaging and actual packaged executable launch pass |
| iOS build | Xcode 26.6 builds unsigned arm64/x86_64 simulator App.app; no UI/lifecycle execution claimed |

See [Android evidence](android-verification.md) for prerequisites, local SDK
override, Gradle cache recovery, and device commands. `node scripts/verify-desktop.mjs`
launches the actual packaged desktop executable and checks its smoke result.
See [Linux evidence](linux-verification.md) for isolated prerequisites, native
package checks, independent conformance and the oversized-request fix.
See [hosted CI evidence](hosted-ci-verification.md) for the verified revision,
all 11 job results, macOS launch and iOS build scope.

CI runs compiler tests on Windows/Linux/macOS and Node22/24, builds/launches
desktop packages, builds Android debug APKs, and builds unsigned iOS simulator
apps. Desktop Linux CI uses Ubuntu 22.04 to preserve Chromium's sandbox without
changing Ubuntu 24's user-namespace security policy; the runner
[retires on April 17, 2027](https://github.com/actions/runner-images/issues/14254).
Before then, migrate to a newer runner with an app-specific sandbox prerequisite.
WSLg evidence does not verify a standard Ubuntu 24 desktop installation subject
to the [AppArmor user-namespace restriction](https://github.com/electron/electron/issues/41066).
Workflow configuration is not evidence of executed checks. Release 1.0,
store distribution/signing, physical devices, older runtime floors, and hidden
conformance remain pending; no unavailable check is counted as passed.
