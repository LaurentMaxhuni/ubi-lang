# Android native verification

Verification date: 2026-10-09. Scope: local Capacitor debug APK and existing
Android emulator. Browser previews do not count as native verification.

## Prerequisites

| Requirement | Evidence |
| --- | --- |
| Node/npm | Node 22.22.1; npm 11.19.0 |
| Capacitor | Core/Android/CLI 8.5.3; Preferences 8.0.1; project lockfile |
| Android Studio | Installed 2026.1.4, build AI-261.26222.65.2614.16204760 |
| Android SDK | Existing platforms `android-36.1`, `android-37.0`; build tools 36.0.0, 36.1.0, 37.0.0 |
| ADB/emulator | ADB 37.0.1-15733141; emulator 37.1.11.0 |
| Java | Project-local Temurin 21.0.12.1+1; installed Oracle/Studio JDKs are 25 |
| Emulator | Existing `Medium_Phone`, Android 17/API 37, x86_64, 16 KB pages; WHPX acceleration |
| SDK licenses | Existing licenses retained; no license acceptance performed |
| Signing/distribution | Debug signing only; release keys/accounts/store publishing not required for this local check |
| Apple targets | Unchecked: this Windows host has no macOS/Xcode build host |

Temurin archive came from the [official Adoptium API](https://api.adoptium.net/v3/assets/latest/21/hotspot?architecture=x64&image_type=jdk&os=windows&vendor=eclipse).
SHA256 checked before extraction:
`f9d6e191ab098c0d416e7d588a24420a8621cd2f4720dab2459b8b7b2d2d8b4e`.
Archive/extracted JDK remain inside ignored `.ubi-build/tools`.

[Capacitor 8 requirements](https://capacitorjs.com/docs/updating/8-0) specify
Node 22+, Android Studio 2025.2.1+, API 24 minimum, SDK 36, AGP 8.13.0, and Gradle
8.14.3. Java 21 runs this Gradle version; see the
[Gradle Java compatibility matrix](https://docs.gradle.org/current/userguide/compatibility.html).

## Emulator isolation

Commands below run from the repository root in PowerShell. Native tools require
the reviewed execution environment on this host; sandboxed Java launch hangs.
Environment variables are process-local; global Java/Android settings remain
unchanged. Gradle cache belongs to `.ubi-build/gradle`.

```powershell
$sdkRoot = 'C:/Users/STORM/AppData/Local/Android/Sdk'
$adbPath = "$sdkRoot/platform-tools/adb.exe"
$env:JAVA_HOME = (Resolve-Path '.ubi-build/tools/jdk-21.0.12.1+1').Path
$env:GRADLE_USER_HOME = (Resolve-Path '.ubi-build').Path + '/gradle'
Start-Process "$sdkRoot/emulator/emulator.exe" -WindowStyle Hidden `
  -ArgumentList @('-avd', 'Medium_Phone', '-read-only', '-no-snapshot',
    '-no-window', '-no-audio', '-no-boot-anim', '-port', '5580') `
  -RedirectStandardOutput '.ubi-build/android-verification/emulator.stdout.log' `
  -RedirectStandardError '.ubi-build/android-verification/emulator.stderr.log'
& $adbPath -s emulator-5580 shell getprop sys.boot_completed
# 1
& $adbPath -s emulator-5580 shell getprop ro.build.version.release
# 17
& $adbPath -s emulator-5580 shell getprop ro.build.version.sdk
# 37
```

Read-only launch and disabled snapshot loading/saving preserve the user's AVD
disk state. Every device command explicitly selects task emulator `emulator-5580`.
Initial cold boot took several minutes; it first reported ADB `offline`, then
`sys.boot_completed=1`. See [Android emulator command-line documentation](https://developer.android.com/studio/run/emulator-commandline).

## Native package generation

These commands completed successfully. The generated project contains the
Preferences Android plugin and packaged web assets/modules:

```powershell
$env:CARGO_TARGET_DIR = (Resolve-Path '.ubi-build').Path + '/android-target'
cargo run -- package --root examples/v1 --target mobile
Set-Location examples/v1/.ubi-dist/mobile
$env:CI = '1'
npm.cmd ci --no-audit --no-fund
npm.cmd run android
```

CI mode disables Capacitor telemetry. Generated outputs/dependencies are ignored.

## Debug APK build

The installed SDK has API 36.1, while Capacitor's template requests API 36.
An ignored Gradle init script selects installed API 36.1/build tools 36.0.0
for every Android module, preserving the template target SDK 36 and minimum
SDK 24. This is a local verification override; generated application sources
and committed packaging templates retain their defaults. The expanded
[AGP 8.13 compile SDK API](https://developer.android.com/reference/tools/gradle-api/8.13/com/android/build/api/dsl/CompileSdkSpec)
supports minor API levels.

`.ubi-build/android-verification/installed-sdk.gradle`:

```groovy
gradle.beforeProject { project ->
    project.afterEvaluate {
        def android = project.extensions.findByName('android')
        if (android != null) {
            android.compileSdk {
                version = release(36) { minorApiLevel = 1 }
            }
            android.buildToolsVersion = '36.0.0'
        }
    }
}
```

```powershell
Set-Location examples/v1/.ubi-dist/mobile/android
$env:JAVA_HOME = 'C:/github-projects/ubi-lang/.ubi-build/tools/jdk-21.0.12.1+1'
$env:GRADLE_USER_HOME = 'C:/github-projects/ubi-lang/.ubi-build/gradle'
$env:ANDROID_HOME = 'C:/Users/STORM/AppData/Local/Android/Sdk'
.\gradlew.bat --no-daemon --max-workers=2 '-Pandroid.builder.sdkDownload=false' `
  -I 'C:/github-projects/ubi-lang/.ubi-build/android-verification/installed-sdk.gradle' `
  assembleDebug
```

The wrapper downloaded Gradle 8.14.3 into the project cache. `--no-daemon`
uses a temporary single-use build process that exits on completion. SDK downloads
are disabled. Final build completed: `BUILD SUCCESSFUL in 19s`, 123 actionable
tasks (22 executed, 101 up-to-date). The first attempts hit the known Windows
[Gradle immutable-cache move issue](https://github.com/gradle/gradle/issues/31438).
After each terminated build, completed transforms with metadata/results were
moved to their intended task-local cache directory without overwriting existing
entries; no security settings, global caches, or SDK licenses changed.

## Executed native behavior

The debug APK at `.ubi-dist/mobile/android/app/build/outputs/apk/debug/app-debug.apk`
installed successfully on `emulator-5580`. `org.ubilang.tasks/.MainActivity`
launched the real native Capacitor app; UIAutomator observed its WebView and
accessible task controls.

Verified APK SHA256:
`03fb0cd740a2fa03e2a33932302905d8de68ea03cf7ae0acbc049f9c8c5d9b2e`.

- Entered `Verify Android Ubi`; Add produced `0 of 1 completed` and task controls.
- Complete produced `1 of 1 completed`; the task's Complete control became disabled.
- Home/background followed by activity resume preserved the completed task.
- `am force-stop org.ubilang.tasks` followed by a fresh launch preserved the same
  task and completion count, proving storage survives process termination.
- Screenshot/UI evidence is saved in ignored `.ubi-build/android-verification/`:
  `android-completed.png`, `ui-resumed.xml`, `ui-relaunched.xml`.

Commands used after package/build:

```powershell
& $adbPath -s emulator-5580 install -r examples/v1/.ubi-dist/mobile/android/app/build/outputs/apk/debug/app-debug.apk
& $adbPath -s emulator-5580 shell am start -n org.ubilang.tasks/.MainActivity
& $adbPath -s emulator-5580 shell uiautomator dump /sdcard/ubi-v1.xml
# Observed bounds drive native test input; synthetic task data only.
& $adbPath -s emulator-5580 shell input keyevent 3
& $adbPath -s emulator-5580 shell am start -n org.ubilang.tasks/.MainActivity
& $adbPath -s emulator-5580 shell am force-stop org.ubilang.tasks
& $adbPath -s emulator-5580 shell am start -n org.ubilang.tasks/.MainActivity
```

API24 devices, physical hardware, release signing/store distribution, and iOS
remain unchecked. The read-only test emulator is stopped after evidence capture;
existing AVD snapshots/data remain unchanged.
