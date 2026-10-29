# Ubi

Native Rust-powered UI is available through the optional `native-ui` feature:
`cargo run --locked --features native-ui -- ui examples/gallery.ubi --root ubi-ui`.
See [Ubi UI native core](ubi-ui/README.md) for the source package, component gallery,
architecture, documentation portal and current limitations.

Build shared applications for web, CLI, desktop, and mobile. The compiler emits
JavaScript ES modules; explicit host adapters provide storage and network access.
V1 integration includes async/await, project creation, formatting, and production
host packages. Release readiness and unexecuted platform checks are tracked in
[platform evidence](docs/PLATFORMS.md).

Create a four-target application or run the persistent reference app:

```powershell
cargo run -- init my-app
cargo run -- dev --root my-app
# Or use the checked-in reference app:
cargo run -- dev --root examples/v1
```

The [portable app](examples/v1/README.md) shares task creation, validation,
completion, deletion, and filtering in Ubi. Hosts preserve saved and visible data
on failed writes and corrupt reads. Package each host:

```powershell
cargo run -- package --root examples/v1 --target web
cargo run -- package --root examples/v1 --target cli
node examples/v1/.ubi-dist/cli/run.mjs add "Try portable Ubi"
node examples/v1/.ubi-dist/cli/run.mjs list
cargo run -- package --root examples/v1 --target desktop
cargo run -- package --root examples/v1 --target mobile
cargo run -- fmt --root examples/v1 --check
cargo run -- --version
```

Outputs live in the project's `.ubi-dist/<target>`. Desktop bundles use Electron;
mobile bundles use Capacitor and native Preferences storage. Native dependency
versions are pinned with per-target lockfiles. Follow the example's commands to
install dependencies and create native packages. Node 22.12+ and npm 10+ are
required for native package tooling. Android and iOS additionally need their
SDKs/build hosts. Packages remain unsigned; no store release is claimed.

Ubi source files use `.ubi`. The Rust compiler checks source and emits JavaScript
ES modules; supported language behavior is documented in [SPEC.md](SPEC.md).

Inspect a checked public interface without running code or generating artifacts:

```powershell
cargo run -- inspect main.ubi --root examples/foundations --json
```

The versioned JSON lists reachable modules and exact source revisions, imports,
exported functions/records/enums, resolved types, generic constraints, async flags,
declared effects and adjacent `///` documentation. Private implementations are
omitted. Invalid programs return the existing compiler diagnostics instead of
partial summaries. Nominal types include their defining source ID. The library's
filesystem-independent `Compiler::inspect()` returns the same serializable summary.
See [the summary contract](SPEC.md#checked-module-summaries-2026-10-29).

Current support includes typed variables/functions, records, immutable tuples/lists,
`Option`/`Result`, generics, immutable maps, user-defined enums and exhaustive matching, loops/ranges, snapshot
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
uses exact `tag`/`values` envelopes. Console/storage host access uses opaque
capabilities and checked function effects. Async functions support immediate
await with checked storage/HTTP effects. Classes and tuples remain future slices.

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

## Generics, errors, maps, and host access

```ubi
record Box<T> { value: T }
fn identity<T>(value: T) -> T { value }
fn parsed(text: string) -> Result<int, string> {
    match (parseInt(text)) {
        Option.Some(value) => Result.Ok(value),
        Option.None => Result.Err("Invalid integer"),
    }
}
fn doubled(text: string) -> Result<int, string> {
    let value = parsed(text)?;
    Result.Ok(value * 2)
}
```

Generic arguments follow inputs and expected types; generic bodies cannot assume
arithmetic/equality support for T. `?` unwraps Ok or returns Err from its named
function with the same error type. Concrete exported wrappers define JavaScript
host argument types.

`Map<K,V>` accepts int/bool/string keys. `mapEmpty`, `mapSet`, `mapGet`,
`mapHas`, `mapRemove`, `mapKeys`, `mapValues`, and `length` preserve immutable
values and insertion order. Map host arguments use serialized entry arrays;
Result uses the enum tag/values envelope.

Run focused examples:

```powershell
cargo run -- run main.ubi --root examples/generics
cargo run -- run main.ubi --root examples/results
cargo run -- run main.ubi --root examples/maps
cargo run -- build main.ubi --root examples/host
node examples/host/run.mjs
```

The [host example](examples/host/README.md) grants console output and an
in-memory string store. Ubi receives `Console`/`Storage` tokens explicitly;
named functions declare `effects(console, storage)`. Trusted adapters return
typed Result JSON. Missing effects, effectful callbacks, and capability escapes
are compile errors. CLI JSON never grants authority. See [contracts](SPEC.md#17-basic-generics-2026-10-05).

## Typed function values

Use `fn(int) -> int` to annotate pure callbacks, return captured functions, or
store functions in records, lists, Options, Results and map values:

```ubi
fn add(amount: int) -> fn(int) -> int {
    (value: int) => value + amount
}

fn twice(transform: fn(int) -> int, value: int) -> int {
    transform(transform(value))
}
```

Ubi modules can export/import these signatures. Public host calls require
signatures without function values; use a concrete Ubi wrapper for host entry.
Captures remain snapshots, calls share runtime budgets, and effectful/async
functions cannot become callbacks. Run the [function-values example](examples/function-values/README.md)
with `cargo run -- run main.ubi --root examples/function-values`; it returns `28`.
See [the function-value contract](SPEC.md#23-typed-pure-function-values-2026-10-14).

Tuples group different types without a record declaration: `(int, string)` and
`(3, "ready")`. Destructure with `let (count, label) = pair;` or nested match
patterns. A singleton needs a trailing comma; `()` remains unit. The
[tuple example](examples/tuples/README.md) combines generic imported functions,
destructuring and Result tuple payloads:

```powershell
cargo run -- run main.ubi --root examples/tuples
```

`T: Equatable` permits equality in generic functions and aggregates while
rejecting function/capability-containing instantiations:

```ubi
fn equal<T: Equatable>(left: T, right: T) -> bool {
    left == right
}
```

The [constrained collection example](examples/constraints/README.md) shares list
membership, deduplication and frequency counting across element types. Run it
with `cargo run -- run main.ubi --root examples/constraints`.

## Run a program

Async named functions declare their resolved return type. `run` awaits async
exports; Ubi callers immediately await direct async calls:

```ubi
async fn twice(value: int) -> int { value * 2 }
export async fn main() -> int { await twice(21) }
```

Use `(await operation())?` to propagate a Result. Async storage and HTTP require
explicit capabilities and declared effects; cancellation comes from the host's
AbortSignal. See the [async example](examples/async/README.md) and
[language contract](SPEC.md#21-v1-asynchronous-application-profile-2026-10-09).

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
Ubi creates/updates each record; this demo keeps its collection in JavaScript.
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

All targets produce JavaScript ES modules. `build` emits shared logic; `package`
adds the project's public interface and selected web/CLI/Electron/Capacitor host.
`init` supplies complete hosts for a new application. The config validates target
names, source paths, and schema version.
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
