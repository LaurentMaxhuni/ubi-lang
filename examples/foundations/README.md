# Programming foundations

Run from repository root with Node.js on PATH:

```powershell
cargo run -- run main.ubi --root examples/foundations
```

Expected output:

```text
foundations: total=30, range=10, captured=6, parsed=42, clamped=9, letters=a|😀|b
```

`main.ubi` demonstrates immutable `List<T>` values, `Option<T>` and exhaustive
matching, mutable local variables, ranges, `for` and `while`, closures with
snapshot captures, `map`/`filter`/`fold`, Unicode scalar strings, strict parsing,
and math utilities. `mathSample` returns `12`:

```powershell
cargo run -- run main.ubi --root examples/foundations --function mathSample
```

Use `List<T>` for ordered homogeneous values. `xs[index]` returns `Option<T>`;
`Option.None` handles negative and out-of-bounds indices. `append` creates a
list; `set` returns `Option<List<T>>` without changing the original. Empty lists
need a known element type, such as `let values: List<int> = [];`.

Function parameters are explicitly typed; local closure result types are
inferred. Captures preserve values from closure construction. Named functions
can also serve as collection callbacks. `return` inside a callback exits that
callback; `break` and `continue` target the innermost loop.

Strings iterate and index by Unicode scalar, including emoji. Numeric parsing
consumes the entire text and returns `Option`; invalid text never partially
parses. Filesystem access, maps, classes, tuples, and user generics are deferred.
