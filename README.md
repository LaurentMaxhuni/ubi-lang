# Ubi

Ubi source files use `.ubi`. The Rust compiler checks source and emits JavaScript
ES modules; supported language behavior is documented in [SPEC.md](SPEC.md).

Current support includes typed variables/functions, records, immutable lists,
`Option`, user-defined enums and exhaustive matching, loops/ranges, snapshot
closures, collection helpers, and pure string/math/conversion utilities. Record fields and collection
elements stay immutable; updates create new values:

```ubi
record Task { title: string, completed: bool }
fn complete(task: Task) -> Task {
    Task { ...task, completed: true }
}
```

The example project includes `createTask` and `completeTask`. JavaScript hosts
pass record arguments as JSON text and receive frozen field objects:

```js
import { createTask, completeTask } from "./.ubi-build/web/src/main.mjs";
const task = createTask(1, "Try Ubi");
const done = completeTask(JSON.stringify(task));
// task.completed remains false; done.completed is true.
```

Primitive arguments pass directly. Record, list, Option, and enum arguments use
serialized JSON; returned aggregates are frozen. See [host encoding](SPEC.md#15-programming-foundations-2026-10-03)
for validation and representation; [enum encoding](SPEC.md#16-user-defined-enums-2026-10-04)
uses exact `tag`/`values` envelopes. User generics, Result, maps,
classes, tuples, async, and host capabilities remain future slices.

```powershell
cargo run -- check main.ubi --root path/to/project
cargo run -- build main.ubi --root path/to/project
```

## Programming foundations

Run the [foundation example](examples/foundations/README.md):

```powershell
cargo run -- run main.ubi --root examples/foundations
```

```ubi
export fn main() -> int {
    let values = [1, 2, 3, 4];
    let doubled = map(values, (value: int) => value * 2);
    let selected = filter(doubled, (value: int) => value > 4);
    let mut total = 0;
    for (value in selected) {
        total = total + value;
    }
    total
}
```

The result is `14`. Indexing returns `Option<T>`; handle both outcomes:

```ubi
fn firstOrZero(values: List<int>) -> int {
    match (values[0]) {
        Option.Some(value) => value,
        Option.None => 0,
    }
}
```

Use `let values: List<int> = [];` for an empty list. `append` and `set` create new
lists; `map`, `filter`, `find`, and `fold` process them with typed callbacks.
`for` accepts lists, Unicode scalar strings, and lazy `range(start, end)` values.
`while`, `break`, and `continue` support ordinary iterative programs.

String helpers include `length`, `contains`, `startsWith`, `endsWith`, `trim`,
`split`, `join`, `replace`, and scalar-indexed `slice`. Math includes `abs`,
`min`, `max`, `clamp`, rounding, square roots, powers, trig, `log`, and `exp`.
Conversions are explicit: `toFloat`, `toInt`, `parseInt`, `parseFloat`, `toString`.
Parsing and float-to-int conversion return Option; `toString` accepts int, bool,
or string. See [the contracts](SPEC.md#15-programming-foundations-2026-10-03).

## User-defined enums

Declare typed variants and handle every case:

```ubi
enum Shape { Point, Rectangle(int, int) }
fn area(shape: Shape) -> int {
    match (shape) {
        Shape.Point => 0,
        Shape.Rectangle(width, height) => width * height,
    }
}
```

Enums support imports, nested patterns, immutable payloads, and structural
equality. Run the [enum example](examples/enums/README.md):

```powershell
cargo run -- run main.ubi --root examples/enums
```

It prints `enums: rectangle=12, total=22`.

## Run a program

Define an exported entry function in `main.ubi`:

```ubi
export fn main() -> string {
    "Hello from Ubi!"
}
```

Compile and execute it with Node.js installed on PATH:

```powershell
cargo run -- run main.ubi
```

The command prints `Hello from Ubi!`. After installing the CLI with
`cargo install --path .`, use `ubi run main.ubi` directly.
Select another exported function or pass JSON arguments:

```powershell
cargo run -- run --root examples/project --target cli --function greeting
cargo run -- run --root examples/project --target cli --function createTask --args '[1,"Try Ubi"]'
```

`run` builds fresh modules before invoking the function. Config selection works
like `build`; omit the entry to use `ubi.json`. Strings print as text, other
values print as JSON, and unit prints nothing. Record inputs use serialized JSON
strings inside the argument array. Compiler/runtime failures exit nonzero.

## Develop a web app

Start the clickable task demo:

```powershell
cargo run -- dev --root examples/project
```

Open `http://127.0.0.1:3000/` in your browser. Add a task and complete it;
both operations call compiled Ubi functions. Edits to `.ubi`, HTML, CSS, and
JavaScript rebuild and reload the page. Compilation errors appear in the page;
fixing them restores the app. Press Ctrl+C to stop. Use `--port 3001` if needed.
Node.js is not required for `dev`.

For your own project, put the interface in `web/index.html`, with CSS/JavaScript
alongside it. Import generated Ubi modules using
`/__ubi/modules/<source-path>.mjs`. For example, `src/main.ubi` becomes
`/__ubi/modules/src/main.mjs`. Run `ubi dev` after installing the updated CLI,
or `cargo run -- dev main.ubi` for an explicit entry.

The server listens only on localhost and serves public web assets and current
generated modules. The demo's list lives in browser memory and resets on reload;
Ubi creates/updates each record, while collection support remains future work.
Config changes require restarting the server.

## Project config

For a complete animated landing page with a live Ubi-powered orbit demo, run:

```powershell
cargo run -- dev --root examples/landing-page --port 3017
```

Open `http://127.0.0.1:3017/`. See the [landing page example](examples/landing-page/README.md)
for source and interaction details.

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
