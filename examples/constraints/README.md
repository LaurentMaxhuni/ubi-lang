# Equality-constrained collection functions

```powershell
cargo run -- run main.ubi --root examples/constraints
```

Expected result:

```text
constraints: pear, apple, plum; pear=3, apple=2, plum=1; pear=true
```

`includes`, `unique` and `frequencies` share one implementation across equatable
element types. `T: Equatable` permits equality in their universally checked
bodies. Inference preserves the bound through imports, tuples, a fold callback
and a captured key. Functions and function-containing elements are rejected.

These helpers use immutable lists and linear scans; they preserve first-seen
order. `Equatable` follows ordinary equality: NaN is unequal to itself, so
deduplication retains repeated NaNs. It grants no arithmetic or ordering on T.
