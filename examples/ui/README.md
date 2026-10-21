# Ubi UI foundation

Editable Ubi components for web, mobile and desktop. The explorer's component
composition, state, themes and event handling are authored in Ubi. A small
JavaScript renderer maps typed views to native HTML controls; Electron and
Capacitor provide desktop/mobile app shells.

From the repository root, with the Ubi binary on PATH:

```powershell
ubi check --root examples/ui
ubi dev --root examples/ui --port 8792
ubi package --root examples/ui --target web
ubi package --root examples/ui --target desktop
ubi package --root examples/ui --target mobile
```

For a source checkout use `cargo run --` before the command, for example
`cargo run -- dev --root examples/ui --port 8792`. Packages are written beneath
`examples/ui/.ubi-dist/`. Web output is a standalone static directory with relative
assets. Serve it over HTTP; ES modules do not work through `file://`.

## Components you can edit

`src/ui/core.ubi` defines the view/event contract and theme/layout enums.
`src/ui/components.ubi` defines `text`, `heading`, `column`, `row`, `card`, `badge`,
`button`, `themed` and `pressed`. Copy these two files into your project's source
tree to start using them, along with the renderer, codec, validator and styles
from `web/`. Integrate through the concrete exports shown in `src/main.ubi`.
There is no `ubi ui init/add` command yet; existing files are never installed or
overwritten automatically.

```ubi
import { View, ButtonVariant } from "./ui/core.ubi";
import { button } from "./ui/components.ubi";

export enum Message { Save }
export fn saveButton() -> View<Message> {
    button("save", "Save changes", ButtonVariant.Primary, false, false, Message.Save)
}
```

Import types from `core.ubi` and helpers from `components.ubi`. Passive generic
components use the surrounding `View<Message>` annotation for type inference.
Every node key must be stable and unique within its mount. The host rejects
invalid graphs before changing the interface.

The app exports concrete `initial`, `validate`, `update` and `view` functions.
The renderer invokes public compiled wrappers, encoding aggregates with the
bounded Ubi ABI codec. Application semantic validation is mandatory; a type-correct
state can still contain invalid domain values.

Theme colors/spacing/radii live in `web/style.css`. `themed(view, Theme.System)`
follows the system setting without global document theme mutations. Layout rules
for the explorer use its node keys; reusable components use the `ubi-*` classes.
Controls support visible focus, native keyboard activation, disabled/busy states
and optional pressed semantics. This slice has no persistence, forms, overlays,
native OS widgets or external framework adapter.

## Native packages

Desktop: in `examples/ui/.ubi-dist/desktop`, run `npm ci --no-audit --no-fund`,
then `npm start` or `npm run package`. `npm run smoke` checks the actual compiled
Ubi state transitions, theme, composition and renderer sandbox/asset isolation.

Mobile: in `examples/ui/.ubi-dist/mobile`, run `npm ci --no-audit --no-fund`,
then `npm run android` or `npm run ios`. These generate/sync the platform project;
building Android requires Java/Android SDK, and iOS requires macOS/Xcode.
Use existing [platform setup](../../docs/PLATFORMS.md). No dependencies are
installed by the Ubi package command itself.

The existing package format requires `storage.js` and the mobile storage adapter;
they are retained for compatibility and never imported by this pure UI explorer.
No storage/device/network capability is given to the component library.

## Checks

From the repository root:

```powershell
cargo test --lib ui_tests
cargo test --test ui_package
node --test examples/ui/tests/codec.test.mjs examples/ui/tests/validate.test.mjs
node examples/ui/tests/serve.mjs
```

Open the printed browser harness URL. It reports graph/lifecycle failures and
includes a keyboard probe: Enter and Space must increment once each. These
checks complement independent interpreter/backend tests; Node checks alone do
not establish focus, responsive layout or accessibility behavior.

Actual platform results and remaining limitations are recorded in
[verification evidence](../../docs/ui-foundation-verification.md).
