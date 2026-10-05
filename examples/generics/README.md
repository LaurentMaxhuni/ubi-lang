# Generic functions and data

From the repository root:

```powershell
cargo run -- run main.ubi --root examples/generics
```

Prints `generics: first=3`. Functions, records, and enums infer their type
arguments from values and expected types. Generic bodies cannot assume concrete
operations such as integer arithmetic. Export concrete wrappers for JavaScript
hosts; generic exports are callable from Ubi modules.
