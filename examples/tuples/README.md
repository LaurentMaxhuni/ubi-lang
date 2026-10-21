# Tuples and destructuring

Run from the repository root:

```powershell
cargo run -- run main.ubi --root examples/tuples
```

Expected result: `tuples: quotient=3, remainder=2, swapped=right, count=3`.

The imported generic `swap` function accepts and returns structural tuple types.
`divide` returns both quotient and remainder inside a Result; matching destructures
the tuple payload. Tuple bindings are immutable. Use `(value,)` for a singleton;
`(value)` groups an expression and `()` remains unit.
