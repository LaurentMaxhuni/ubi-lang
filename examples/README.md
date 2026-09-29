# Public conformance fixtures

Milestone 0 review artifacts for [SPEC.md](../SPEC.md). No compiler or interpreter exists yet. These expectations were hand-derived from the specification by a separate corpus author; source generation, source hashing and byte-offset calculations did not use implementation output. Independent review and human sign-off are recorded in [DECISIONS.md](../DECISIONS.md).

The corpus contains 40 cases (19 accepted source graphs, 21 rejected source graphs), 64 selected exported invocations, and 43 source files. Minimum milestones mark the first profile that must support a case. Milestone 2 fixtures are future contracts; their presence does not assert implemented support.

## Manifest schema, version 1

[conformance.json](conformance.json) is UTF-8 JSON with these top-level fields:

- `schemaVersion`: integer 1.
- `specification`: corpus-relative specification path.
- `offsetUnit`: `"utf8-byte"`.
- `sourceRevisionAlgorithm`: `"sha256"`.
- `cases`: array of the case objects below.

Every case has:

- `id`: unique stable case label.
- `minimumMilestone`: integer 1 or 2.
- `rootSourceId`: root module's source ID.
- `sources`: nonempty array of `{ "id": "...", "path": "...", "revision": "sha256:..." }`. Paths are relative to this directory; IDs are case-sensitive, root-relative compiler source IDs. A case supplies exactly these module bytes to the compiler library. The revision is SHA-256 of the original bytes, including final newlines.
- `check`: either `{ "outcome": "accept" }` or `{ "outcome": "reject", "requiredDiagnostics": [...], "allowAdditionalDiagnostics": true }`.
- `invocations`: present only for accepted cases; array of named invocations with `entrypoint`, `arguments`, and `expected`.

An invocation's `entrypoint` is `{ "sourceId": "...", "export": "..." }`; `name` is a unique label within that case. Its arguments are typed serialized values in declaration order. Its expectation is either `{ "kind": "value", "value": <typed value> }` or `{ "kind": "fault", "code": "UBI-R0001" }`. Fault matching requires the stable code, no normal value, and a nonempty emitted fault message. Runtime wording and source location are not matched.

A required diagnostic is `{ "code": "UBI0010", "severity": "error", "primary": { "sourceId": "main.ubi", "start": 48, "end": 55 } }`. The illustrative offsets here are not a fixture expectation. Real offsets are in the manifest. Spans are zero-based half-open UTF-8 byte offsets; zero-width EOF spans are valid. Each required diagnostic must appear. Additional legitimate diagnostics are allowed. Message wording, related spans and suggested edits are not matched, but emitted diagnostics must still satisfy SPEC.md's full envelope and field contract.

Accepted graphs must have no errors; legitimate warnings/notes are allowed. Reject every invalid graph before executing either backend. Every accepted invocation must execute independently through the interpreter and generated JavaScript and satisfy its specified expected result on both. Backend agreement alone cannot pass an expectation. Milestone 0 performs structural validation only; no executable conformance or hidden evaluation has occurred.

## Types and typed values

A type descriptor is recursively one of:

- Primitive string: `"int"`, `"float"`, `"bool"`, `"string"`, or `"unit"`.
- Prelude aggregate: `{ "name": "List", "arguments": ["int"] }`, `Option` with one argument, or `Result` with two.
- Nominal type: `{ "sourceId": "main.ubi", "name": "Point", "arguments": [] }`. Declaration identity uses this source ID within the case. Generic arguments are descriptors in declaration order.

Typed values use these shapes:

| Value | JSON fields |
| --- | --- |
| int | `{ "type": "int", "value": 42 }`; integer in signed 32-bit range |
| bool | `{ "type": "bool", "value": true }` |
| string | `{ "type": "string", "value": "😀" }`; Unicode scalar sequence, no normalization |
| unit | `{ "type": "unit" }` |
| float | `{ "type": "float", "value": "1.5" }`; decimal or special string below |
| list | `{ "type": <List<T>>, "elements": [<T value>, ...] }` |
| record | `{ "type": <nominal record>, "fields": { "x": <typed value>, ... } }` |
| enum | `{ "type": <enum type>, "variant": "Some", "payload": [<typed value>, ...] }` |

Each value carries its complete type. List elements, record fields and enum payloads recursively carry types consistent with the enclosing instantiated declaration. Records include exactly the declared fields; field-object ordering is immaterial. Enums include a qualified type identity, an unqualified variant name, and ordered positional payloads; payload-free variants use an empty array. No null, missing elements, host objects, closures or capabilities occur in the fixtures. Input decoding must obey SPEC.md's trusted host boundary.

Float special strings are exactly `"NaN"`, `"+Infinity"`, `"-Infinity"`, `"+0"`, and `"-0"`. A finite nonzero decimal string is converted to binary64 with nearest-ties-to-even rounding. A float expectation compares the resulting binary64 value including zero sign; any NaN payload/sign satisfies `"NaN"`. Integer JSON values are never float values. Oracle matching compares serialized value structure and floats by this rule; it does not apply Ubi's `==` to expected results, since NaN must remain representable.

For example, a present scalar is:

```json
{
  "type": { "name": "Option", "arguments": ["string"] },
  "variant": "Some",
  "payload": [{ "type": "string", "value": "😀" }]
}
```

## Coverage

Core fixtures cover precedence; signed division/remainder and minimum int; immutable scope and shadowing; inferred private returns, explicit exported returns, unit and early return; recursion; binary64 rounding, underflow, NaN, infinities and signed zero; short-circuit branches; first-fault argument/operand order; two-module imports.

Rejected fixtures cover lexical/EOF errors; unsupported syntax; numeric literal range; UTF-8 unknown-name spans; duplicate/immutable bindings; return annotations; argument count; path/export/cycle resolution; private signatures; nominal equality; field mutation/duplicates; guarded exhaustiveness; Result error mismatch; captured rebinding; unresolved generic arguments and unknown variants.

Shared-domain fixtures cover Unicode scalar indexing and absent indices; structural equality and aggregates containing NaN; record/list copies that preserve aliases; field/spread-base order; append/set/index; snapshot captures and closure-owned local rebinding; named map callbacks, filter order and first callback fault; nested enum patterns and guard order; generic nominal values and instantiated captures; same-error Result propagation and explicit mapping. Task-list invocations exercise valid inputs and domain errors.

## Task-list contract

[task-list.ubi](task-list.ubi) is an executable pure module for Milestone 2. Public data:

- `Task { id: int, title: string, completed: bool }`.
- `TaskError { EmptyTitle, InvalidId, DuplicateId(int), MissingId(int) }`.

| Export | Contract |
| --- | --- |
| `validateTitle(title) -> Result<string, TaskError>` | Empty scalar sequence yields EmptyTitle. Any nonempty string succeeds unchanged, including whitespace and combining characters. No trim or normalization. |
| `create(tasks, id, title) -> Result<List<Task>, TaskError>` | Validate title, then reject a negative ID, then reject an ID already present. Append one incomplete task; preserve existing values and order. Errors are EmptyTitle, InvalidId, then DuplicateId(id) in that priority. |
| `complete(tasks, id) -> Result<List<Task>, TaskError>` | Missing ID yields MissingId(id). Otherwise mark every task with that ID complete, preserving titles, IDs, other tasks and order. Completing an already-complete task succeeds. |
| `filterTasks(tasks, completed) -> List<Task>` | Retain original tasks whose completion flag matches, in original order. |
| `example() -> Result<List<Task>, TaskError>` | Create IDs 1 and 2, then complete ID 1; executable composition of the public API. |

IDs are caller-supplied. Starting from a valid list with unique nonnegative IDs, create preserves that invariant. Functions accept any well-typed list; complete explicitly updates all matches if a caller supplies duplicates. Empty input is supported. Every operation preserves its input values. Persistence and platform capabilities belong to later application/host layers.

All source fixtures use LF UTF-8 without BOM. Do not regenerate expected outcomes from a compiler. Updating bytes requires recomputing source revisions and reviewing affected diagnostic offsets and behavior expectations. Static JSON, hash and span checks establish artifact consistency only.
