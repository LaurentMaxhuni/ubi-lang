# Ubi Tasks

Shared rules live in `src/main.ubi`. Web, Node CLI, Electron desktop and Capacitor
mobile hosts call the same compiled module. No UI framework.

Requires Ubi on PATH. Node **22.12+**, npm **10+** for hosts/package tooling.
Browser support contract: Chromium 120+, Firefox 121+, Safari 17.2+. Desktop: Windows 10+,
macOS 13+, or an Electron-supported Linux distribution. Mobile: Android 7/API24+
or iOS 15+. Dependency versions are exact and locked separately per native target.

```sh
ubi dev --root . --target web
ubi package --root . --target web
ubi package --root . --target cli
node .ubi-dist/cli/run.mjs add "Ship one small thing"
node .ubi-dist/cli/run.mjs list active
node .ubi-dist/cli/run.mjs complete 1
node .ubi-dist/cli/run.mjs delete 1
```

Serve `.ubi-dist/web` through an HTTP server. Packaged imports are relative, so
deployment under a subdirectory works. CLI storage defaults to `tasks.json`
beside `run.mjs`; override with `--store path`. Reads decode through concrete Ubi
types and validate task rules. Mutations take an exclusive `.lock`, write an
exclusive temporary file, flush, then rename. Corruption, unavailable storage,
or a failed mutation exits 1 without replacing prior persisted data. An existing
lock blocks concurrent mutations; remove a stale lock only after its writer has
stopped. No CLI output is treated as a successful mutation until rename succeeds.

```sh
ubi package --root . --target desktop
cd .ubi-dist/desktop
npm ci
npm run package
npm run smoke
```

`out/UbiTasks-<platform>-<arch>` contains the actual native executable and Electron
runtime. Launch `UbiTasks.exe` on Windows (or the platform's app/executable). Pass
`--smoke-test` to verify a hidden native window loads compiled rules, adds a task,
and has no renderer Node access; smoke uses a separate temporary storage profile.
Normal launch uses Electron's application storage. Renderer sandbox and context
isolation stay enabled; new windows, permissions and remote navigation are denied.
Native packages are unsigned. Scripts never publish or access signing credentials.

```sh
ubi package --root . --target mobile
cd .ubi-dist/mobile
npm ci
npm run android
# Or on macOS: npm run ios
npm run open:android
```

Mobile output includes `www` compiled assets, a bundled Preferences storage
adapter, locked packages, Capacitor config and `native.mjs`. `android`/`ios`
generate a native project if absent, then sync; `sync` updates existing projects.
Generation/tool failures return nonzero; prerequisite failures return 2. Android
build/run requires Android Studio 2025.2.1+, SDK/platform 36, build tools, JDK 21
and an emulator/device. JDK 21 is the compatible recommended toolchain; newer
JDKs need Gradle compatibility checks. iOS requires macOS, Xcode 26+ and simulator/device. Native
storage uses Capacitor Preferences and reloads when the app resumes; browser
Preferences fallback uses local storage. Native build/device checks are separate
from source generation, and must be recorded for a release.

Sources: [Electron security](https://www.electronjs.org/docs/latest/tutorial/security),
[Electron Packager](https://github.com/electron/packager),
[Capacitor requirements](https://capacitorjs.com/docs/getting-started/environment-setup),
[Capacitor workflow](https://capacitorjs.com/docs/basics/workflow),
[Preferences storage](https://capacitorjs.com/docs/apis/preferences).
