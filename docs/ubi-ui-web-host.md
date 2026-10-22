# Shared Ubi components on web, mobile and desktop

The user requires language syntax highlighting and working web controls, with
shared source for web, mobile and desktop. Static native captures remain useful
evidence, but cannot serve as the web interaction implementation.

## Implementation

- Compile the existing forty component examples and gallery with the existing
  checked JavaScript backend. Preserve their actual Ubi init/update/view reducers.
- Add a reusable, framework-independent browser host for the current schema-1
  View. Use semantic HTML controls, keyed updates, bounded validation, scoped CSS,
  native keyboard/touch/input behavior and transactional model/view commits.
- Export standalone static applications with relative module/assets paths. The
  same exported web directory can be hosted on the web or bundled in a WebView;
  the existing Rust desktop host continues using the same Ubi source directly.
- Embed live browser applications in documentation previews. Keep native images
  as an explicitly named alternative. Synchronize theme without resetting state,
  provide reset/open-example actions, and display load/runtime errors honestly.
- Highlight Ubi source as escaped React text spans, preserving exact source and
  copy behavior. Match the actual lexer, including comment/string boundaries.

## Verification

Compile every example, compare emitted JavaScript initial values/views and event
transitions with native interpreter results, validate malicious/malformed views,
exercise all forty mounts and representative controls in a real browser, check
keyboard/focus/edit retention, themes, CSS isolation and narrow layouts. Build
the static docs, run relevant Node/Rust checks and independent review, then commit.

WebView compatibility is a packaging contract. Real Android/iOS execution of this
expanded library is reported only if exercised; browser viewport checks alone do
not establish device verification or native screen-reader integration.
