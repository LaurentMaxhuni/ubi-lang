# Ubi language specification

Version: 0.2 draft. Current milestone: 1. Milestone 0 was accepted; compiler implementation is in progress. Review evidence is recorded in DECISIONS.md.

## 1. Scope and conformance

Milestone 1 implements the core profile: primitive values, immutable local bindings, operators, named functions, calls, `if`, blocks, returns, and named imports/exports. Milestone 2 adds `let mut`, records, enums, lists, generics, `match`, `?`, and pure closures. Examples identify their required milestone; later-profile examples are contracts, not claims of current support. Effects and foreign access have the boundary contract in section 8; their adapter format is specified before Milestone 3 implementation. Async, entrypoints with host capabilities, and platform behavior require later specification revisions.

A conforming implementation preserves specified values, evaluation order, recoverable errors, and fatal faults on every backend. Invalid programs are rejected before either execution path runs. Compiler optimizations cannot change observable behavior. The interpreter evaluates source syntax independently of JavaScript lowering; agreement alone does not establish correctness. Public expectations and an evaluator-owned hidden corpus establish expected behavior separately.

## 2. Source and grammar

Source is UTF-8 without a byte-order mark. Identifiers are ASCII `[A-Za-z_][A-Za-z0-9_]*`, case-sensitive. `_` alone is reserved for wildcard patterns. Keywords cannot be identifiers; lex them as a distinct token kind rather than as `IDENT`. Whitespace is exactly U+0009 tab, U+000A line feed, U+000D carriage return, and U+0020 space. Newlines have no special syntax. `//` comments end before line feed or carriage return; `/* ... */` comments are non-nesting. Unterminated comments/strings and invalid source encoding are lexical errors. Retain comments and their original byte spans for future formatting.

Integer tokens are decimal digits. Float tokens have a decimal point with digits on both sides, an exponent, or both; exponent syntax is `[eE][+-]?[0-9]+`. Leading zeroes are permitted, with no octal interpretation. Signs are separate operators. Numeric separators, hexadecimal literals, interpolation, and single-quoted strings are unsupported. Strings use double quotes and permit `\"`, `\\`, `\n`, `\r`, `\t`, and `\u{H...}` (1-6 hexadecimal digits denoting a Unicode scalar). Reject unknown escapes, surrogate escapes, raw newlines, and scalars above U+10FFFF. Raw Unicode scalars other than quotes/backslashes/newlines are allowed. No normalization occurs.

EBNF: `{ x }` means repetition, `[ x ]` optional, `|` alternatives. Quoted text is a token; `IDENT`, `INT`, `FLOAT`, and `STRING` are lexical tokens. Comma-separated nonempty sequences may have one trailing comma wherever shown below. `name` excludes keywords.

```ebnf
module       = { import }, { declaration } ;
import       = "import", "{", name, { ",", name }, [ "," ], "}", "from", STRING, ";" ;
declaration  = [ "export" ], ( function | record | enum ) ;
function     = "fn", name, [ generics ], "(", [ parameters ], ")", [ "->", type ], block ;
generics     = "<", name, { ",", name }, [ "," ], ">" ;
parameters   = name, ":", type, { ",", name, ":", type }, [ "," ] ;
type         = primitive_type | name, [ "<", type, { ",", type }, [ "," ], ">" ] ;
primitive_type = "int" | "float" | "bool" | "string" | "unit" ;
record       = "record", name, [ generics ], "{", [ fields ], "}" ;
fields       = name, ":", type, { ",", name, ":", type }, [ "," ] ;
enum         = "enum", name, [ generics ], "{", variant, { ",", variant }, [ "," ], "}" ;
variant      = name, [ "(", type, { ",", type }, [ "," ], ")" ] ;
block        = "{", { statement }, [ expression ], "}" ;
statement    = "let", [ "mut" ], name, [ ":", type ], "=", expression, ";"
             | name, "=", expression, ";"
             | "return", [ expression ], ";"
             | expression, ";" ;
expression   = unary, { binary_op, unary } ; (* precedence table below *)
unary        = ( "-" | "!" ), unary | postfix ;
postfix      = primary, { "(", [ arguments ], ")" | ".", name
             | "[", expression, "]" | "?" } ;
arguments    = expression, { ",", expression }, [ "," ] ;
primary      = INT | FLOAT | STRING | "true" | "false" | name | "(", ")"
             | "(", expression, ")" | block | if_expr | match_expr | arrow
             | "[", [ arguments ], "]" | record_value ;
if_expr      = "if", "(", expression, ")", block, "else", ( block | if_expr ) ;
record_value = name, "{", [ record_items ], "}" ;
record_items = [ "...", expression, "," ], field_value, { ",", field_value }, [ "," ] ;
field_value  = name, ":", expression ;
arrow        = "(", [ parameters ], ")", "=>", ( expression | block ) ;
match_expr   = "match", "(", expression, ")", "{", arm, { ",", arm }, [ "," ], "}" ;
arm          = pattern, [ "if", "(", expression, ")" ], "=>", expression ;
pattern      = "_" | name | INT | "-", INT | STRING | "true" | "false"
             | name, ".", name, [ "(", pattern, { ",", pattern }, [ "," ], ")" ] ;
```

`name` denotes `IDENT`. `binary_op` is one of the operators in the table. Enum construction is qualified member access, optionally called with payloads, e.g. `Result.Ok(3)`. Resolve it as a constructor, not an ordinary object field. Generic arguments on value expressions are inferred from arguments and expected types; explicit call-site type arguments and function types in source are deferred. `()` is the unit literal; `unit` is its type. Empty records use `Name {}`. Updates require at least one replacement field and a trailing comma after the spread base.

Arrow recognition uses the following `=>` to distinguish typed parameter lists from grouped expressions. A `{` following a bare name starts a record value; required parentheses around `if`/`match` conditions prevent ambiguity. An arrow's block is the ordinary block expression, with no separate alternative interpretation. Member/index/call/`?` suffixes bind in source order, before prefix operators. All binary operators associate left; two ordering/equality operators cannot occur in the same unparenthesized chain.

| Precedence, highest first | Operators |
| --- | --- |
| Postfix | call, `.`, `[]`, `?` |
| Prefix | `-`, `!` |
| Multiplicative | `*`, `/`, `%` |
| Additive | `+`, `-` |
| Ordering | `<`, `<=`, `>`, `>=` |
| Equality | `==`, `!=` |
| Boolean conjunction | `&&` |
| Boolean disjunction | `\|\|` |

Assignment is a statement, not an expression. Record/field/index assignment, `++`, compound assignment, loops, `null`, `undefined`, `any`, casts, classes, exceptions, async/await, dynamic imports, and ambient host globals are unsupported. Keywords `async`, `await`, `throw`, `try`, `catch`, `class`, `while`, `for`, `break`, `continue`, `new`, `null`, `undefined`, `any`, and `as` are reserved and receive an unsupported-feature diagnostic rather than acquiring JavaScript semantics. Other keywords are the quoted words in the grammar plus `int`, `float`, `bool`, `string`, and `unit`; these five are permitted in the `type` production only.

## 3. Bindings, functions, and modules

- Bindings have lexical block scope from the end of their initializer. `let` cannot be reassigned. Milestone 2's `let mut` permits rebinding a local name to the same type; it never permits changing a field, list element, or aliased value. Parameters and pattern bindings are immutable. Duplicate names in one scope are errors; nested scopes may shadow outer names. Initializers see outer bindings, not the name being declared. Functions cannot be declared inside blocks.
- Function parameters always have explicit types. Exported functions and recursive functions (including every member of a mutual recursion cycle) require return annotations; other private functions infer one return type from all reachable returns and normal completion. Generics are invariant, named explicitly on declarations, and checked for all possible substitutions; operations requiring a particular type cannot be applied to an unconstrained type parameter. Infer value-expression type arguments from arguments and expected types, including annotated binding initializers and function result expressions; reject unresolved type arguments with `UBI0020` on the expression. No coercion, overloads on user functions, subtyping, default arguments, or variadic calls. Calls require exactly the declared argument count.
- A block's trailing expression is its value. A block without one yields `unit`. A semicolon discards an expression value. `return;` returns unit; `return value;` exits the innermost function/arrow. Every reachable completion/return path must agree with the function's return type. A path that returns or faults contributes no value to its enclosing expression. `if` requires a `bool`, requires `else`, and unifies the types of completing branches.
- Module scope has only imports and declarations, with no execution or initializers. Declarations are visible throughout their module, allowing named recursion. Functions, record types/constructors, and enum types/constructors become public only through `export`. Fields and variants of exported types are public. An exported signature cannot mention a private nominal type. Record field types and enum payload types count as exported signatures. Built-in types/functions are a prelude available in every module; reserved type names and prelude symbols cannot be redeclared or imported under the same name.
- Imports are explicit named imports from a literal relative `.ubi` path starting `./` or `../`. Normalize separators to `/` and resolve `.`/`..` against the importing source ID within the supplied project root. Reject absolute paths, backslashes, escapes above the root, missing sources/exports, duplicate imported names, and cyclic imports. Source IDs are case-sensitive root-relative paths on every host. No implicit extensions, aliases, re-exports, package resolution, or filesystem access inside the compiler library. The CLI is responsible for supplying sources without following paths outside its project root.
- Both backends evaluate the same selected exported function with typed input supplied by a trusted test/host harness. Milestone 1 has no implicit `main`, top-level effects, or ambient globals. Defining a function does not execute its body. Milestone 1 calls only named functions; function values/pure arrows arrive in Milestone 2.

## 4. Values and operators

| Type / operation | Required behavior |
| --- | --- |
| `int` | Signed 32-bit, -2147483648 through 2147483647. Decimal literals outside range are compile errors. Unary `-` with the integer token `2147483648` as its direct syntactic operand forms the minimum literal, ignoring intervening whitespace/comments. Thus `- 2147483648` is valid; `-(2147483648)` is invalid because the positive token is grouped. The positive token anywhere else is invalid. Signed integer patterns follow the same range/whitespace rules. Negating minimum or overflowing `+`, `-`, `*` causes `UBI-R0001`. Never wrap or silently promote. |
| Integer `/` | Truncate toward zero. Zero divisor causes `UBI-R0002`; minimum divided by -1 causes `UBI-R0001`. Result remains `int`. |
| Integer `%` | Remainder has dividend's sign, satisfies `a = (a / b) * b + a % b` when division is defined. Zero divisor causes `UBI-R0002`; minimum % -1 is 0 without overflow. |
| `float` | IEEE 754 binary64, round to nearest ties to even for literals and each arithmetic operation. Literal overflow is a compile error; underflow rounds to subnormal/zero. Runtime operations may produce signed zero, infinities, and NaN. Unary `-` and binary `+`, `-`, `*`, `/` follow these rules; unary `+` is unsupported and `%` is int-only. No fused operations or reassociation that change results. NaN payload/sign is unobservable. |
| Float comparisons | NaN equals nothing, including itself. Ordering with NaN is false; `!=` is true. Positive and negative zero compare equal. Division by signed zero produces signed infinity, or NaN for zero/zero. |
| `bool` | Only `true`/`false`. `!`, `&&`, `\|\|` accept bools; no truthiness. |
| `string` | Immutable sequence of Unicode scalar values. `+` concatenates two strings. `length(text)` counts scalars. `text[index]` returns `Option<string>` containing exactly one scalar, or `Option.None` for negative/out-of-range int indices. No UTF-16 indexing, normalization, implicit numeric conversion, or ordering comparisons. |
| `unit` | Single value `()`, equal to itself. |

Arithmetic and ordering require operands of the same numeric type; no int/float mixing. Equality requires identical types and is value equality for primitives, lists, records, and enums. Lists compare length and elements in order; nominal records compare all declared fields; nominal enums compare variant and payloads. Different nominal types never compare, even with identical fields. An aggregate containing a NaN can be unequal to itself. Function/closure/capability values, including aggregates containing them, are not equatable. An unconstrained generic parameter does not guarantee equality support. No reference identity or object pointer equality is exposed.

JavaScript output must check integer arithmetic and division, preserve binary64 operation boundaries, and implement Unicode scalar indexing and structural equality explicitly. Using unchecked JavaScript operators for these cases does not satisfy this specification.

## 5. Immutable aggregates and closures (Milestone 2)

- `record Name { field: Type, ... }` declares a nominal type identified by canonical source ID and declaration name. `Name { field: value, ... }` must provide each field exactly once, with no unknown fields. Evaluate fields in written order. `Name { ...base, field: value, ... }` requires a base of the identical instantiated record type; evaluate base once, then replacement expressions in written order. Unmentioned fields keep their values. The result is a new immutable record; every existing alias remains unchanged.
- `[a, b, ...]` creates `List<T>`; all elements have identical type. Empty lists require an expected element type. List indexing accepts int and returns `Option<T>`; negative/out-of-range indices yield `Option.None`. Lists never grow through index assignment. `length(values)` returns int; `append(values, item)` returns a new list; `set(values, index, item)` returns `Option<List<T>>` with a copied update or `Option.None`. A list/string exceeding int's maximum length causes `UBI-R0003`.
- **v0 representation: plain copying.** List append/set/map/filter copy the resulting element sequence; record construction/update copies field slots. Immutable child values may be shared because identity is unobservable. Append/set cost O(n) slots; record update O(number of fields). Repeated append can be quadratic. No persistent collection library, shared mutable objects, or copy-on-write mutation is required. Change representation only through a recorded, conformance-preserving decision based on profiling.
- Built-in signatures are `length(string) -> int`, `length(List<T>) -> int`, `append(List<T>, T) -> List<T>`, `set(List<T>, int, T) -> Option<List<T>>`, `map(List<T>, pure (T) -> U) -> List<U>`, and `filter(List<T>, pure (T) -> bool) -> List<T>`. Function types here describe built-in contracts; they are not source type syntax. Built-ins become available with their owning profile (`length` on strings is Milestone 2). No other collection/string operations are implicit.
- Resolve `length` statically from its argument's established type: string or List<T>. Neither result context nor runtime data selects an overload. `length([])` lacks an element type and is `UBI0020` on `[]`, even though both overloads return int; an explicitly typed empty list is valid. `length(x)` for unconstrained generic T is invalid (`UBI0020` on x); `length(xs)` for List<T> is valid. No further overloads or inferred type defaults exist.
- `(x: int) => x + 1` is a pure closure with inferred return type. Parameter types are explicit. Named pure functions may also be passed to map/filter. Arrows snapshot free local values when created, including the then-current values of `let mut` bindings. Rebinding an outer local afterwards does not affect its captured value. Captures cannot be reassigned inside the closure; closure-owned `let mut` locals may be rebound. Closures cannot capture capabilities, including inside aggregates, or call an effectful function directly/indirectly. Named function references are resolved statically, not captured mutable bindings. Generic closure captures receive their instantiated types.
- Map/filter invoke callbacks once per element, left to right. Map retains order; filter retains the original values for true results. They stop at the first fatal fault. Source-level function values may be inferred local bindings or built-in callback arguments, but cannot appear in exported signatures or be stored in records/lists in this version. A closure cannot contain `?` in this version because an explicit Result return annotation for arrows is deferred.

## 6. Enums, matching, and recoverable errors (Milestone 2)

Enums are nominal tagged values with positional payloads. Construct `Color.Red` for a payload-free variant and `Option.Some(value)` for a variant with payload. Constructor payload count/types must match; infer generic arguments from payloads or expected types. Define these prelude enums exactly:

```ubi
enum Option<T> { Some(T), None }
enum Result<T, E> { Ok(T), Err(E) }
```

`match (value) { pattern => expression, ... }` evaluates its subject once, tries arms in source order, and evaluates only the chosen arm. Patterns bind immutable names in that arm; nested enum patterns are permitted. Repeated binding names in one pattern are invalid. Literal patterns require the same subject type. An enum pattern must use the subject's nominal enum and correct payload arity/types. Matching an `int`, string, record, list, float, unit, or generic subject requires a wildcard/binding arm for exhaustive coverage; record/list/float/unit destructuring/literal patterns are deferred. Bool coverage is both literals or a catch-all. Enum coverage includes every variant's possible nested payload values. Guards require bool and never add exhaustive coverage. All completing arms have the same type. Reject non-exhaustive matches; an unreachable later arm may warn but is not an error.

`value?` is valid only for a `Result<T, E>` inside a named function with explicit return annotation `Result<U, E>`. Evaluate value once. `Result.Ok(x)` yields x; `Result.Err(e)` immediately returns `Result.Err(e)` from that function. Error types must be identical, including nominal identity and generic arguments. `?` on Option, outside a Result-returning function, or with a different error type is rejected. No From-style conversion. Map different errors explicitly with `match` and construct the receiving function's error variant. Revisit conversion at Milestone 2 through `DECISIONS.md` before any change. `Result` describes recoverable failures; it does not catch integer overflow, resource exhaustion, or adapter defects.

## 7. Evaluation order and fatal faults

Evaluate operands, callee then arguments, list elements, and record fields strictly left to right, exactly once. Resolve assignment target before evaluating its right side. `&&` skips its right operand when left is false; `||` skips it when left is true. `if` evaluates its condition once and only its selected branch; `match` checks a guard only after its pattern matches. The first fault or early return stops subsequent evaluation. Rebinding occurs only after the right side completes. Lowering must preserve these rules, including field updates, `?`, and nested calls.

A fatal fault ends the current exported invocation with a stable runtime code; no normal value is returned, and later operations do not run. Already-completed host effects are not rolled back. The CLI host reports the fault on stderr and exits nonzero; embedded hosts receive a structured fault separate from Result values. Minimum fault data is `{ "code": "UBI-R0001", "message": "..." }`; source location may be attached but is not required for runtime corpus matching. Host exception text/stack is not the stable interface. Faults cannot be caught in ordinary Ubi code.

Runtime codes: `UBI-R0001` integer overflow, `UBI-R0002` integer division/remainder by zero, `UBI-R0003` unrepresentable length, `UBI-R0004` unexpected trusted-adapter fault, `UBI-R0005` runtime resource exhaustion. The future compiler must publish finite input/nesting/work limits and diagnose exhaustion with `UBI0090`; a watchdog timeout is an evaluation failure, never success. Exact budgets belong to the Milestone 1 implementation decision. Backend/host memory exhaustion is a fatal resource fault, not a fabricated Result. Host process termination may prevent reporting and counts as an environmental failure.

## 8. Host boundary and unsupported operations

Ordinary Ubi code has no ambient `window`, `document`, `process`, console, filesystem, network, clock, randomness, or arbitrary JavaScript import. Named `.ubi` imports resolve only Ubi modules. Unsupported host access is rejected before execution. Trusted JavaScript hosts validate inbound arguments recursively: numeric kind/range, booleans, Unicode strings, record fields, enum tags/payloads, and lists. Wrong/missing/extra record fields, sparse lists, malformed tags, lone surrogates, cycles, functions, getters/proxies, and unexpected host objects are invalid data; never run getters to decode input. Adapters decode serialized input or known adapter-owned data instead of inspecting arbitrary live host objects; they cannot promise to detect a malicious JavaScript proxy safely. Copy accepted data into immutable Ubi values. Adapters must distinguish int and float using the declared type, since a JavaScript number alone carries no Ubi type.

Milestone 3 adapters must return a typed decoding error for malformed foreign results and normalize documented recoverable exceptions into the declared Result error. Unexpected exceptions or contract violations cause `UBI-R0004`. Reject malformed inbound invocation arguments before executing Ubi code; that is a host validation failure, not a Ubi value. Trusted adapters implement the boundary and can violate it; effects do not isolate malicious JavaScript dependencies.

The Milestone 3 permission contract requires host-created opaque capability values and finite effects on named functions, checked transitively through calls and recursion. Ordinary Ubi cannot construct capabilities; an effectful operation needs both its matching scoped capability and a declared effect. Pure functions/arrows cannot call effectful functions indirectly through aliases or imported names. Capability scope controls authority; effects describe possible operations. Adapter declaration syntax, allowed operations, and effect syntax are intentionally deferred to a pre-implementation specification revision. Async/cancellation/JSON decoder behavior is likewise unsupported until specified; no JavaScript fallback is allowed.

## 9. Diagnostics and edits

Versioned JSON diagnostic envelope (a check with no diagnostics still emits this envelope with an empty array):

```json
{
  "schemaVersion": 1,
  "offsetUnit": "utf8-byte",
  "sources": [{ "id": "main.ubi", "revision": "sha256:<64 lowercase hex digits>" }],
  "diagnostics": [{
    "code": "UBI0010",
    "severity": "error",
    "message": "Unknown name: missing",
    "primary": { "sourceId": "main.ubi", "start": 32, "end": 39 },
    "related": [],
    "edits": [{
      "span": { "sourceId": "main.ubi", "start": 32, "end": 39 },
      "sourceRevision": "sha256:<64 lowercase hex digits>",
      "replacement": "existing",
      "applicability": "maybe-incorrect"
    }]
  }]
}
```

Required diagnostic fields: code, severity (`error`, `warning`, `note`), nonempty human message, primary span, related spans. `related` entries have `{ "span": <span>, "message": <nonempty text> }`. `edits` is optional; absence means no suggestion. Sources include every supplied module, sorted by source ID; each revision is SHA-256 of exact original bytes, including line endings. Primary and related spans refer to these IDs. All spans are zero-based, half-open UTF-8 byte offsets `[start, end)`, with `0 <= start <= end <= byteLength`; empty spans mark insertion points/EOF. Never use UTF-16 indices. Valid-text spans end on scalar boundaries; malformed-encoding diagnostics may point at invalid bytes.

Sort diagnostics by source ID, primary start, primary end, code, then message (strings use lexicographic UTF-8 byte order; offsets use numeric order). Sources use the same string ordering. Codes/severity/spans are normative; wording may improve without changing the schema. An error prevents building/executing any module in that graph; warnings/notes do not. Check/build exit zero on success and nonzero on errors. JSON diagnostics go to stdout with no human text mixed in; human diagnostics go to stderr. Artifact placement and operational CLI exit-code categories are specified in Milestone 1.

An edit's `sourceRevision` must match both the envelope source revision and current file bytes. Apply a diagnostic's edits atomically after checking all sources/revisions/ranges; reject stale edits, overlaps, duplicate insertion points, invalid UTF-8 boundary positions, and replacements containing invalid Unicode scalars without modifying any source. `machine-applicable` means the compiler considers that suggestion safe, not permission to apply it; `maybe-incorrect` requires human review. Related spans and suggestions may be omitted from example expectations, but required diagnostic fields cannot be omitted from emitted output.

| Code | Meaning and required primary span |
| --- | --- |
| `UBI0001` | Invalid token/encoding/escape or unterminated lexical item; offending bytes/token, unterminated item's opening through EOF. |
| `UBI0002` | Invalid grammar or missing token; unexpected token, or zero-width EOF when input ends. |
| `UBI0003` | Unsupported feature; feature keyword/token, or unsupported assignment's entire target (e.g. `task.title`). |
| `UBI0004` | Numeric literal outside representable range; entire literal, including a direct unary negative sign and intervening whitespace/comments when applicable. |
| `UBI0010` | Unknown name; unresolved identifier. |
| `UBI0011` | Duplicate name/declaration/import; second name occurrence. |
| `UBI0012` | Missing/invalid module, missing export, or import cycle; import path token for module/path/cycle errors, imported name for missing export. Cycle reports earliest cycle-edge import by source ID and byte offset, with other edges as related spans. |
| `UBI0013` | Private type exposed by exported signature; offending type name in signature. |
| `UBI0020` | Type mismatch or invalid operator operand; incompatible expression (right operand for differing binary types; operand for unsupported unary use). |
| `UBI0021` | Missing required return annotation; function name. |
| `UBI0022` | Assignment to immutable binding/capture; assignment target name. |
| `UBI0023` | Invalid argument/payload count; entire call/construction. |
| `UBI0030` | Invalid/missing/duplicate record field or invalid field access; unknown/duplicate field name, entire constructor for missing field, accessed field name for invalid access. |
| `UBI0031` | Invalid pattern/variant/payload; offending pattern or constructor. |
| `UBI0032` | Non-exhaustive match; `match` keyword. |
| `UBI0033` | Invalid propagation context/error type; `?` token. |
| `UBI0040` | Missing capability/effect or effectful closure; offending call/capture. Reserved for Milestone 3 checking. |
| `UBI0090` | Compiler resource limit exceeded; consuming construct, or zero-width offset 0 if no construct can be decoded. |

Select lexical/unsupported/parse errors before name/type errors for the same offending construct; do not manufacture semantic diagnostics from a failed parse. Unsupported assignment targets are recognized as `UBI0003`, even though not valid statement productions. Otherwise multiple independent errors may be reported. Invalid corpus cases state the required primary error; additional legitimate errors are permitted unless the case explicitly requires an exact list.

## 10. Review artifacts

`examples/conformance.json` lists public sources, required profiles, entrypoints, typed arguments, and expected values/faults or required compile diagnostics. Float expectations use tagged strings for NaN, infinities, and signed zero so JSON does not lose information; nominal aggregates identify their module/type, enum variant, and payload. The corpus README defines the oracle format. Public `.ubi` fixtures use LF line endings, enforced by `.gitattributes` so checkout conversion cannot invalidate hashes/spans; the compiler still accepts CRLF and counts its exact bytes. `examples/task-list.ubi` declares the intended pure domain interface, with executable example bodies; persistence belongs to later application/host layers.

Human sign-off in `DECISIONS.md` must name the reviewed Git revision and evidence, approve examples and decisions, and explicitly accept Milestone 0's gate. A separate evaluator owns hidden tests outside implementation workspace/credentials. Do not treat public static validation as compiler conformance or claim the hidden evaluation has occurred.
