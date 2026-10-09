# Platform readiness

Scope: shared Ubi applications on web, Node CLI, desktop, and Android/iOS mobile.
"All versions" is interpreted as these platform targets, with supported runtime
floors rather than arbitrary historical browser or OS versions.

## Local prerequisite inventory, 2026-10-09

| Requirement | Available evidence | Status / owner |
| --- | --- | --- |
| Rust compiler and Cargo | Rust/Cargo 1.97.1 installed | Ready; agent |
| Node and npm | Node 22.22.1, npm 11.19.0 installed | Ready; agent |
| Browser web host | Browser automation available | Verification pending; agent |
| Windows desktop build/launch | Windows host; Electron package work underway | Verification pending; agent |
| Android SDK | SDK root at `%LOCALAPPDATA%/Android/Sdk`; platforms 36.1/37.0, build-tools 36.0.0/36.1.0/37.0.0 | Ready; agent |
| Android emulator | Emulator 37.1.11; Medium_Phone and Pixel_10_Pro_XL AVDs | Ready; agent |
| Java | Oracle 25.0.1 and Android Studio JBR 25.0.3 | Gradle compatibility check pending; agent |
| Android debug signing | Generated debug signing, no developer account required | Local route only; agent |
| macOS/Xcode build host | No macOS execution host available in this session | blocked-on-human; user supplies host or CI execution |
| Linux desktop build/launch | No Linux execution host available in this session | blocked-on-human; user supplies host or CI execution |
| iOS simulator/device | Requires macOS/Xcode and simulator/device | blocked-on-human; user supplies host |
| Store distribution/signing | No signing credentials requested or inspected | Not required for local debug/portable packages; required before store release |
| Hidden conformance | Evaluator-owned corpus unavailable | blocked-on-human; independent evaluator |

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

V1 application/package checks are in progress. Package generation alone does
not establish native launch, lifecycle, installation, accessibility, or store
distribution. Add exact commands and outcomes as they are executed.
