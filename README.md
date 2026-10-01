# Ubi

Ubi source files use `.ubi`. The Rust compiler checks source and emits JavaScript
ES modules; supported language behavior is documented in [SPEC.md](SPEC.md).

```powershell
cargo run -- check main.ubi --root path/to/project
cargo run -- build main.ubi --root path/to/project
```

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
