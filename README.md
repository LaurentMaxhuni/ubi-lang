# Ubi

Ubi source files use `.ubi`. The Rust compiler checks source and emits JavaScript
ES modules; supported language behavior is documented in [SPEC.md](SPEC.md).

```powershell
cargo run -- check main.ubi --root path/to/project
cargo run -- build main.ubi --root path/to/project
```

## Project config

Put `ubi.json` in an application's root to declare where its shared logic runs:

```json
{
  "schemaVersion": 1,
  "name": "hello-ubi",
  "entry": "src/main.ubi",
  "targets": ["web", "mobile", "desktop", "cli"]
}
```

Declare one or several targets. Select one when the project has multiple targets:

```powershell
cargo run -- check --root examples/project --target web
cargo run -- build --root examples/project --target web
```

The runnable [example](examples/project/ubi.json) emits its shared modules to
`.ubi-build/web/`. With a single declared target, `--target` is optional. Running
`ubi check` or `ubi build` inside a project reads its `ubi.json`; `--out-dir` can
override the build directory. Explicit source commands above still work.

All targets currently produce JavaScript ES modules. Target declarations describe
the intended application; mobile/desktop packaging, UI, and host integrations are
future work. The config validates target names, source paths, and schema version.
See [the config contract](SPEC.md#13-project-configuration) for details.

## Windows file recognition

Run once from the repository:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/register-windows.ps1
```

Windows recognizes `.ubi` as **Ubi source file**, uses the editor's icon, and
opens files in installed VS Code (or Notepad when VS Code is unavailable).
Choose another editor with `-EditorPath 'C:\path\editor.exe'`; preview registration
with `-WhatIf`. Registration applies to the current user and requires no admin
access. Existing default choices are preserved; use **Open with > Choose another
app** if you want to change them. Opening a file edits source; compile with the
commands above.

Registration follows Microsoft's [file association guidance](https://learn.microsoft.com/en-us/windows/win32/shell/fa-file-types).
