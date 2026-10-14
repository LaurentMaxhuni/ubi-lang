# Typed function values

Run from the repository root:

```powershell
cargo run -- run main.ubi --root examples/function-values
```

Expected result: `28`.

The example returns captured functions from another Ubi module, stores them in a
record and a typed list, and passes them to a function that invokes them twice.
Function signatures use `fn(int) -> int`; calls preserve shared runtime budgets.
These functions remain pure. Public host calls use the concrete `main` wrapper;
function-containing signatures are available to Ubi imports.
