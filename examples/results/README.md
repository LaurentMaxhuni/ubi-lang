# Recoverable errors

```powershell
cargo run -- run main.ubi --root examples/results
```

Prints `result: 15`. `?` unwraps Ok or returns Err from the current named
function. Error types must match; map different errors explicitly with `match`.
Fatal arithmetic/resource faults remain separate from Result.
