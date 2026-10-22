# Ubi UI developer portal

Next.js 16 / React 19 / Tailwind 4. Documentation only; native components live in
the sibling Rust crates and Ubi source package.

```powershell
npm ci
npm run dev
npm run build
```

`build` exports a static site into `out/`; serve that directory with any static
HTTP server. Webpack is selected explicitly because the installed Next 16.4.2
Turbopack worker exits prematurely on this Windows environment. Both the source
and native captures are local; the website needs no external fonts or services.

The API reference and first example are read directly from the sibling source
files during build. Public gallery PNGs come from the actual Ubi application and
wgpu renderer. Regeneration commands and no-overwrite behavior are documented in
the [framework README](../README.md).
