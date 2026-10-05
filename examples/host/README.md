# Explicit host capabilities

From the repository root:

```powershell
cargo run -- build main.ubi --root examples/host
node examples/host/run.mjs
```

Prints two messages, then `{"tag":"Ok","values":["Explicit capabilities"]}`.
The JavaScript host grants console output and an in-memory string store through
opaque tokens. Ubi functions declare the effects they may perform and pass
capabilities explicitly. This example does not persist data across runs.

Adapters return serialized Result envelopes; documented failures return
`{"tag":"Err","values":[{"tag":"Failure","values":["message"]}]}`.
Malformed responses become HostError.Decode; unexpected adapter exceptions fault.
CLI JSON arguments cannot supply a capability. Browser hosts may grant storage
through their own synchronous adapter using the same factories and contracts.
