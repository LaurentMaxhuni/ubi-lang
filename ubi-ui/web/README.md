# Ubi UI browser host

Import `renderer.css` and call `mount(root, compiledApp)` from `renderer.mjs`.
`compiledApp` exposes Ubi's `init`, `update`, and `view` exports. Aggregates cross
the compiled JS ABI through the bounded codec; reducers and component composition
stay in `.ubi` source.
Primitive model arguments cross the public ABI directly; records, lists and
enums are encoded as JSON. The host handles that distinction automatically.

```js
import { mount } from './renderer.mjs';
import * as app from './my-app.mjs';
const handle = mount(document.querySelector('#app'), app, {
  theme: 'system',
  motion: 'system',
  onError: error => console.error(error),
});
// handle.setTheme('light' | 'dark' | 'system');
// handle.setMotion('full' | 'reduced' | 'system');
// handle.getModel(); handle.getView(); handle.dispose();
```

Omit `theme` to follow the source root's Theme property, with system fallback.
Source Theme properties on descendant nodes remain scoped to their subtrees.
`preview: true` hides the example root's direct `title` and `description` children
and centers its component. Default mounts display the complete application.

Nodes reconcile by stable source key and primitive kind. Input identity, focus,
selection, and scroll survive updates. A model commits after its complete View
validates and DOM patch succeeds; failures invoke `onError` and retain the last
working model/UI. Disposed mounts ignore events. Styles apply only inside
`data-ubi-mount`. No HTML or CSS strings supplied by applications are interpreted.

Motion defaults to the system preference. Reduced motion settles finite tracks
and leaves decorative effects static. Source `Prop.Motion` can override timing;
zero duration requests immediate presentation. Animation frames never call Ubi
`update` or `view`. Keyed visual exits omit editable subtrees and have no live
input or accessibility bindings. Dispose each mount when its host is removed.

Trusted embedders can supply `clock: { now, requestFrame, cancelFrame }` for
deterministic tests. `now()` returns monotonic, nonnegative milliseconds;
`requestFrame(callback)` returns a handle accepted by `cancelFrame(handle)`.
This clock is a host interface, not a Ubi capability.

Run `node --test ubi-ui/web/tests/*.test.mjs` from the repository root.
Serve the repository and open `ubi-ui/web/tests/browser.html` for actual browser
regressions (status available as `document.body.dataset.testStatus`). Exported
examples provide additional integration coverage against compiled Ubi reducers.

## Portable distribution

```powershell
cargo run --locked --example ui-web-export -- target/ubi-web
python -m http.server 4319 --bind 127.0.0.1 --directory target/ubi-web
```

Open `/` for links to all seventy components and the gallery. The complete output
is a standalone static directory: `runtime/` contains the host, stylesheet, font
and codec; each application directory contains its own checked JavaScript
modules. Copy this output, rather than isolated repository source files, to a
web host or a mobile/desktop WebView asset directory. There is no runtime Rust
server or React dependency. Serve ES modules over HTTP or the shell's local
asset origin.

For a custom app, compile the same source with `ubi build`, retain its generated
relative module layout, and import its concrete exports in a module that calls
`mount`. Use the exported `runtime/` beside it; the source `codec.mjs` re-export
is a repository convenience, while the distribution includes the full codec.

Mobile shells can point Capacitor's `webDir` at the complete exported directory;
desktop WebViews can serve that directory with script execution isolated from OS
APIs. The existing Rust host remains a separate desktop renderer of the same
Ubi source. This expanded library has browser viewport coverage; physical
Android/iOS execution is not yet verified.
