import fs from "node:fs";
import path from "node:path";
import NativeGallery from "./native-gallery";

const components = [
  ["Window / View", "window / view", "Root canvas and plain containers. Props and child views."],
  ["Stack / Flex", "column / row / flex", "Vertical, horizontal, or wrapping stacks; gap, padding, grow, alignment."],
  ["Grid", "grid", "Equal fraction columns with Columns(1..64), gap and sizing constraints."],
  ["ScrollView", "scrollView", "Vertical scrolling; nested wheel routing, clamped offsets, clipped drawing and hits."],
  ["Card / Separator", "card / separator", "Token-driven surfaces, borders and structural dividers."],
  ["Text / Heading", "text / heading", "Intrinsic measurement and word wrapping; heading levels 1–6."],
  ["Button / IconButton", "button / iconButton", "Pointer release, Enter and Space activate; disabled, busy, pressed, hover, focus. Icon buttons require a label."],
  ["Checkbox / Switch", "checkbox / switch", "Controlled boolean values; pointer, Enter and Space emit Toggle(message, bool)."],
  ["Slider", "slider", "Controlled 0–100 value; drag, arrows, Home and End emit Select(message, string)."],
  ["TextInput / PasswordInput", "textInput / passwordInput", "Controlled single-line editing, scalar-safe caret, Backspace/Delete, Home/End, select all and IME commits."],
  ["Badge / ProgressBar", "badge / progressBar", "Semantic tones and determinate 0–100 progress; no simulated activity."],
];
const sections = [["start", "Getting started"], ["model", "State & events"], ["components", "Components"], ["layout", "Layout"], ["themes", "Themes"], ["gallery", "Native gallery"], ["reference", "API reference"], ["platforms", "Platform status"]];
function Code({ children, language = "ubi" }) { return <div className="code"><div className="code-label">{language}</div><pre><code>{children}</code></pre></div>; }
export default function Page() {
  const core = fs.readFileSync(path.resolve(process.cwd(), "../packages/ubi-ui/core.ubi"), "utf8");
  const api = fs.readFileSync(path.resolve(process.cwd(), "../packages/ubi-ui/components.ubi"), "utf8");
  const counter = fs.readFileSync(path.resolve(process.cwd(), "../examples/counter.ubi"), "utf8");
  return <>
    <a className="skip" href="#content">Skip to documentation</a>
    <header><a href="#" className="wordmark">ubi<span>/</span>ui</a><span className="header-note">Native interface foundations</span><a className="version" href="#platforms">Phase 1 · 0.1.0</a></header>
    <div className="shell">
      <aside><p className="nav-title">Documentation</p><nav aria-label="Documentation sections">{sections.map(([id, title]) => <a key={id} href={`#${id}`}>{title}</a>)}</nav><div className="aside-note">Rust runtime.<br />Ubi applications.<br />Your own canvas.</div></aside>
      <main id="content">
        <div className="intro"><p className="eyebrow">Ubi UI / Native core</p><h1>Interfaces, in<br />your language.</h1><p className="lead">Build a native desktop interface with Ubi. An original component system, retained layout and a Rust GPU renderer bring your application to the screen.</p><a className="start-link" href="#start">Run the native gallery <span aria-hidden="true">↗</span></a></div>
        <figure className="hero-capture"><img src="/native-light.png" width="1100" height="1000" alt="The actual Ubi native gallery with buttons, inputs, switches, slider and layout tokens" /><figcaption>Native renderer capture · Windows · AMD Radeon graphics · 49 Ubi nodes</figcaption></figure>
        <section id="start"><h2>Start with a real window.</h2><p>From the repository root, install Rust and a desktop graphics driver. Node is needed only for this documentation site and existing JavaScript targets.</p><Code language="terminal">{`cargo run --locked --features native-ui -- \\\n  ui examples/gallery.ubi --root ubi-ui`}</Code><p>The optional <code>native-ui</code> feature enables the winit desktop host. Existing compiler commands keep their JavaScript backend.</p><h3>Your first component</h3><Code>{counter}</Code><p>This example is included as <code>ubi-ui/examples/counter.ubi</code>. Pass <code>examples/counter.ubi</code> to the command above. Imports are ordinary relative Ubi source imports.</p></section>
        <section id="model"><h2>State has one owner.</h2><p>The host owns the model returned by <code>init</code>. A control emits a typed event; <code>update</code> returns a new model, and <code>view</code> derives the component tree. The model and validated view commit together. A fault preserves the last working state.</p><div className="flow" aria-label="Event flow"><span>Input event</span><b aria-hidden="true">→</b><span>Ubi update</span><b aria-hidden="true">→</b><span>Keyed view</span><b aria-hidden="true">→</b><span>GPU frame</span></div><p>Equal models skip view evaluation. Stable keys retain layout caches, focus and scroll offsets. Changed models evaluate the Ubi view function once; automatic signal dependency tracking is not implemented. Computed state is an ordinary pure Ubi function.</p><Code>{`checkbox("enabled", "Enable action", model.enabled,
    Message.Enable, [])

// In update:
Event.Toggle(Message.Enable, value) =>
    Model { ...model, enabled: value }`}</Code></section>
        <section id="components"><h2>A useful, tested first set.</h2><p>Every component below is implemented in Rust and callable through the Ubi source package. Props appear before children on containers and last on interactive controls. Each key must be globally unique within a view.</p><div className="table-wrap"><table><thead><tr><th>Family</th><th>Ubi functions</th><th>Behavior</th></tr></thead><tbody>{components.map(([name, fn, behavior]) => <tr key={name}><td>{name}</td><td><code>{fn}</code></td><td>{behavior}</td></tr>)}</tbody></table></div><p>Radio groups, segmented/toggle groups, multiline/numeric/select/combo/search inputs, split views, tabs, navigation, overlays, toasts, spinners, skeletons and avatars remain unimplemented. They are not exported as placeholders.</p></section>
        <section id="layout"><h2>Layout before pixels.</h2><p>Taffy computes layout independently of rendering. Use logical pixels, percentage dimensions, minimum and maximum constraints, flexible growth, grid columns and absolute offsets. A plain View can clip descendants with <code>Prop.Clip(true)</code>.</p><Code>{`grid("settings", [Prop.Columns(2), Prop.Gap(Space.Lg)], [
    card("left", [Prop.MinWidth(240)], children),
    card("right", [Prop.MinWidth(240)], otherChildren)
])

scrollView("body", [Prop.Grow(1), Prop.Height(0)], content)`}</Code><p>Flex wraps at available width. Grid has a fixed column count; choose a different count in Ubi state when needed. The desktop host updates viewport geometry on resize and rasterizes glyphs at the window's DPI. Scrolling is vertical; horizontal scrolling and automatic responsive breakpoints are not yet exposed.</p></section>
        <section id="themes"><h2>One set of tokens.</h2><p>Light and dark themes share semantic text, surface, accent, border, status, radius, spacing and focus tokens. Put a Theme prop on any container to override its subtree; omitted themes inherit. System follows the desktop appearance.</p><Code>{`window("app", [Prop.Theme(Theme.Dark)], children)

button("save", "Save", Message.Save, [
    Prop.Variant(ButtonVariant.Outline),
    Prop.Disabled(!model.ready)
])`}</Code><p>Rust embedders can clone and customize the public <code>ubi_ui_theme::Theme</code> tokens and pass them to the renderer. The Ubi API currently exposes named themes and component props; arbitrary Ubi color-token definitions are not yet available. States update immediately; animated transitions are future work.</p></section>
        <section id="gallery"><h2>The canvas, captured.</h2><p>These images come from the same Ubi application and GPU renderer as the desktop window. Appearance selection switches between captured images; it does not simulate native component interaction.</p><NativeGallery /><Code language="terminal">{`cargo run --locked --features native-ui --example native-ui-capture -- \\\n  ubi-ui/docs/public

# Window creation and GPU presentation smoke check:
cargo run --locked --features native-ui -- \\\n  ui examples/gallery.ubi --root ubi-ui --smoke`}</Code><p>Capture files are created exclusively. Choose an empty output directory to regenerate without overwriting existing evidence.</p></section>
        <section id="reference"><h2>Read the actual API.</h2><p>These declarations are read directly from the Ubi package during the documentation build. No parallel React component implementation exists.</p><details><summary>Core types, properties and events</summary><Code>{core}</Code></details><details><summary>Component function signatures and implementations</summary><Code>{api}</Code></details><h3>Boundary rules</h3><p>Native functions are concrete, exported, pure and synchronous. Arguments are recursively checked against Ubi types. Execution retains the existing one million step, 32 call-frame and 32 aggregate-depth limits. The native JSON boundary limits values to 100,000 and string data to 4 MiB. Views limit nodes to 4,096, depth to 32, keys to 256 bytes and control text to 16 KiB.</p><p>Unknown components, unknown or duplicate properties, bad dimensions, missing keys, cycles, shared children and unreachable nodes reject the complete update. Native exports cannot receive capabilities or function values. Model updates run on the event thread under bounded execution; expensive application work must stay outside these reducers.</p></section>
        <section id="platforms"><h2>What has been verified.</h2><div className="table-wrap"><table><thead><tr><th>Target</th><th>Evidence</th></tr></thead><tbody><tr><td>Windows desktop</td><td>Native window creation and GPU presentation on NVIDIA; native pixel readback and captures on AMD; Rust interaction and Ubi integration tests.</td></tr><tr><td>Linux / macOS desktop</td><td>winit/wgpu architecture supports desktop backends. This native Ubi host has not been executed on these systems.</td></tr><tr><td>WebAssembly / mobile</td><td>Core separation permits future hosts. No native Ubi backend is implemented or verified.</td></tr><tr><td>Accessibility</td><td>Stable roles, names, bounds and control states are exposed as snapshots. OS screen-reader adapters remain unimplemented.</td></tr><tr><td>Text</td><td>Bundled Inter under OFL; shared measurement and glyph cache. Advanced shaping, font fallback, bidirectional text, clipboard and range selection remain limitations.</td></tr></tbody></table></div><p>GPU tests, state/event integration and layout tests are executable. The existing Electron and Capacitor packages retain their separate platform evidence. Phase 1 does not claim production parity with established native toolkits.</p><Code language="terminal">{`cargo test --locked --workspace --features native-ui
cargo clippy --workspace --all-targets --features native-ui -- -D warnings
cargo bench --locked --features native-ui --bench native_ui`}</Code></section>
        <footer><a className="wordmark" href="#">ubi<span>/</span>ui</a><p>Built in Rust. Declared in Ubi.</p><a href="#start">Back to the beginning ↑</a></footer>
      </main>
    </div>
  </>;
}
