# Sequential async hosts

From repository root:

```powershell
cargo run -- build main.ubi --root examples/async
node examples/async/run.mjs
```

Refresh awaits HTTP, then storage write, then storage read. Clear awaits removal.
The host uses deterministic HTTP responses and an in-memory store; no network
access or persistent files are needed. The final refresh receives
`HostError.Cancelled` because its capability's signal has been aborted.

Async host callbacks return Promises of serialized Result JSON. Declared
`storage` and `http` effects require explicit scoped capabilities. Async calls
must be immediately awaited; `(await operation())?` propagates expected errors.
