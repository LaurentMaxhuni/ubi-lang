# Immutable maps

```powershell
cargo run -- run main.ubi --root examples/maps
```

Prints `maps: apples, pears; total=8`. Set/remove return new maps; replacing a key
preserves its position. Get returns Option. Keys are int, bool, or string.
JavaScript hosts encode map arguments as serialized arrays of `[key, value]`
entries; duplicate keys are rejected.
