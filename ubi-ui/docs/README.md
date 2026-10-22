# Ubi UI developer portal

Next.js 16 / React 19 / Tailwind 4. Documentation only; native components live in
the sibling Rust crates and Ubi source package.

```powershell
npm ci
npm run dev
npm run build
```

The live development server uses `http://127.0.0.1:4317`. It reads the current
source and reloads edits automatically. Development output lives in `.next-dev/`
separately from production `.next/`, so building cannot overwrite live dev caches.
Embedded apps use explicit `index.html` URLs supported by both Next's public-file
server and static hosting.

`build` exports a static site into `out/`; serve that directory with any static
HTTP server. Webpack is selected explicitly because the installed Next 16.4.2
Turbopack worker exits prematurely on this Windows environment. Both the source
and native captures are local; the website needs no external fonts or services.

The static portal exports `/`, `/docs`, six guide pages, `/docs/components`, and
40 individual component pages. The grouped sidebar, page outlines, breadcrumbs,
previous/next links, search dialog (Ctrl/Cmd+K), theme switch, copy feedback, and
Preview/Native/Code tabs are documentation controls. Preview executes compiled
Ubi init/update/view functions through the shared browser host; Native retains
the GPU capture. Mobile navigation uses a native browser dialog with focus trapping
and Escape dismissal.

`../catalog.json` is required. Component signatures, implementation source, and
complete runnable examples are read from the Ubi modules at build time. No
placeholder catalog is used. Component-specific usage and accessibility notes
live in `app/component-notes.js`; guide content lives in `app/guide-content.jsx`.

Public gallery PNGs and `public/components/<slug>-{light,dark}.png` come from the
actual Ubi application and wgpu renderer. Generate the component captures before
building or publishing, and check that all 80 files are present. Both appearance
images are selected by the website theme. Regeneration commands and no-overwrite
behavior are documented in the [framework README](../README.md).

`predev` and `prebuild` export all forty browser examples and the gallery into
ignored `public/previews/` using the Rust compiler. These are standalone static
applications, not React replicas. Theme messages preserve model state, Reset
remounts the application, and Open example runs it outside its preview frame.
The deployed site needs no Rust process. Build-time source code is highlighted
with a lossless Ubi tokenizer and accessible light/dark colors; copying retains
the exact original text.

Run `npm test` for tokenizer, palette, ABI and schema checks. The real browser
harnesses are `../web/tests/browser.html` and `../tools/browser_catalog.html` when
the repository is served over HTTP. `cargo test --test web_ui` compares all
compiled applications and event transitions against the native interpreter.

The build uses local Inter under its bundled OFL license, without remote font
requests or runtime content services. On the managed Windows sandbox, Next's
worker/filesystem access may require the existing approved outside-sandbox build
command; an EPERM error during export does not confirm a successful build.
