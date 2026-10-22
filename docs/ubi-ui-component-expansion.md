# Ubi UI component library and documentation expansion

User request: adopt shadcn-style documentation, substantially expand native
components, and separate components individually. Preserve the Rust/Ubi runtime;
React is confined to documentation. Keep existing imports working.

## Deliverables

- Multi-page Next documentation: introduction, installation, state/events,
  layout, theming, accessibility, platforms, component index, individual component
  routes, persistent grouped sidebar, page outline, previous/next navigation,
  keyboard-accessible search, light/dark theme, responsive mobile navigation,
  copyable Ubi source and preview/code tabs.
- Separate Ubi files in `packages/ubi-ui/components/`. Preserve the existing
  `components.ubi` public facade through forwarding functions using existing
  language syntax. Document direct per-component imports.
- Existing native primitives plus Accordion, Alert, Avatar, Breadcrumb,
  ButtonGroup, Collapsible, Empty, Field, Kbd, Label, Link, NumberInput,
  RadioGroup, SearchInput, SegmentedControl, Skeleton, Tabs, TextArea, Toggle and
  ToggleGroup. Composites use the existing retained primitives; new native
  semantics/rendering/input are added where behavior cannot be composed.
- Real per-component Ubi applications with reducer/event coverage and light/dark
  GPU captures. No React reconstructions of native controls or placeholder pages.

## Shared contracts

Native ABI additions: kinds `Radio`, `Tab`, `Toggle`, `ToggleItem`, `Link`,
`TextArea`, `NumberInput`, `Avatar`, `Alert`, `Kbd`, `Skeleton`, `Label`.
Properties `Selected(bool)` (aliases checked state), `Option(string)` (item
selection value), `Group(string)` (roving focus group), `Min(int)`, `Max(int)`,
`Step(int)`, `ReadOnly(bool)`, `Role(string)` (validated semantic override).
Existing Value/Input/Toggle/Change/Activate are reused. Radio/Tab/ToggleItem emit
`Select(message, option)`; Toggle emits `Toggle(message,bool)`; NumberInput emits
`Input(message,string)` and validates/clamps committed arrow-step edits.
TextArea accepts newlines and vertical caret movement. SearchInput composes a
TextInput with search content/label. Skeleton is a static loading surface and
does not pretend to animate. Link dispatches a Ubi message; it grants no browser
or shell authority.

Source catalog: `ubi-ui/catalog.json` is an array of objects with `slug`, `name`,
`category`, `description`, `function`, `module`, `keyboard`, `props` (array of
`name`, `type`, `description`), `examples` (array of `title`, `source` paths
relative to ubi-ui). Each component owns a real example under
`examples/components/<slug>.ubi`. Captures go to
`docs/public/components/<slug>-light.png` and `-dark.png`.

## Verification sequence

1. Extend core validation, keyboard/control semantics, layout/painting, and
   accessibility with focused behavioral regressions.
2. Split public source modules and add real composite/native constructors and
   independent examples. Check every module/example with actual compiler and
   native execution; validate controlled event round trips.
3. Generate per-component captures with the actual GPU renderer. Build docs from
   catalog and checked source. Verify routes, search, copy, navigation, theme,
   component preview/code tabs and narrow browser layout.
4. Run workspace tests, Clippy, formatting, docs build, native window smoke and
   independent review. Commit completed milestones and record exact limitations.
