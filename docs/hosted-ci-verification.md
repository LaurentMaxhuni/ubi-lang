# Hosted CI verification, 2026-10-14

Verified revision: `a6328bcc232849149aaab66a9378ae5cc7a72038`.
[GitHub Actions run](https://github.com/LaurentMaxhuni/ubi-lang/actions/runs/38598365263)
completed successfully: all 11 jobs pass.

| Jobs | Evidence |
| --- | --- |
| Compiler: Windows/Linux/macOS, Node 22/24 (6) | Each runs formatting, denied-warning Clippy, all 180 tests and web/CLI/desktop/mobile packaging |
| Desktop: Windows/Linux/macOS (3) | Native source smoke, packaging and actual packaged executable smoke succeed with UBI_DESKTOP_SMOKE_OK |
| Mobile: Android (1) | Generated Capacitor project builds debug APK; artifact upload succeeds |
| Mobile: iOS (1) | Xcode 26.6 on macOS 26.6.2 builds unsigned arm64/x86_64 simulator App.app; artifact upload succeeds |

The first run failed because `sdkmanager` was absent from PATH. Commit `2b0382e`
uses `$ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager`, matching the
[runner SDK installation](https://github.com/actions/runner-images/blob/main/images/ubuntu/scripts/build/install-android-sdk.sh).

The second run found intermittent HTTP 408 on a valid macOS dev status request.
Accepted sockets could retain nonblocking mode. Commit `a6328bc` restores
blocking I/O before applying existing timeouts. An independent regression
reproduced 408 before the fix and 200 afterward with delayed headers. An idle
connection still returns 408. All 180 local Windows tests, formatting and Clippy
pass; independent final review found no actionable issues.

The iOS job compiles the app; it does not launch a simulator or verify UI,
persistence, suspend/resume or relaunch. Physical devices, required release
signing/store distribution, older runtime floors and evaluator-owned hidden
conformance remain unchecked. Version stays 0.1.0; no 1.0 release is asserted.
