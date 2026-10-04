# User-defined enums

Run from repository root with Node.js on PATH:

```powershell
cargo run -- run main.ubi --root examples/enums
```

Expected output: `enums: rectangle=12, total=22`.

`shapes.ubi` exports a nominal `Shape` enum and exhaustively matches its variants.
`main.ubi` imports that type and function, constructs immutable variants, and
processes a `List<Shape>` using named callbacks. Payloads have declared types and
positions; `Shape.Point` has no payload, while `Shape.Rectangle(3, 4)` has two.

Inspect an exported enum value:

```powershell
cargo run -- run main.ubi --root examples/enums --function rectangle
```

Expected output: `{"tag":"Rectangle","values":[3,4]}`.

Nested enum/Option patterns and guarded arms are supported. Coverage checks
every payload combination; guards require an unguarded fallback. Enum values
compare structurally within the same nominal type. Host arguments use serialized
JSON with exactly `tag` and `values`; returned objects and payload arrays are
frozen. Generic enums and `Result` remain deferred.
