import Link from "next/link";
import { Code, NativePreview } from "./controls";
import { components, readSource } from "./lib/content";

export default function Home() {
  const gallery = readSource("examples/gallery.ubi");
  const tabs = readSource("examples/components/tabs.ubi");
  return <main id="content" className="home-main">
    <section className="home-hero">
      <div className="hero-copy">
        <Link className="release-note" href="/docs/components"><span className="release-dot"/>{components.length} components. Yours to build with.<span aria-hidden="true">›</span></Link>
        <h1>Interfaces,<br/>written in Ubi.</h1>
        <p>Compose the view. Own the state.<br className="desktop-break"/> Take the same source to the web, your phone, and your desktop.</p>
        <div className="hero-actions">
          <Link className="button primary" href="/docs/installation">Start building <span aria-hidden="true">↗</span></Link>
          <Link className="button secondary" href="/docs/components">Explore components</Link>
        </div>
        <div className="hero-detail">
          <Link href="/docs/platforms">Web</Link>
          <Link href="/docs/platforms">Mobile WebViews</Link>
          <Link href="/docs/platforms">Native desktop</Link>
        </div>
      </div>
      <section className="hero-workbench" aria-label="Interactive Tabs example">
        <NativePreview slug="tabs" name="Tabs" code={tabs}/>
        <p className="hero-example-note">A small component, real Ubi state. Try the tabs.<Link href="/docs/components/tabs">Meet Tabs ↗</Link></p>
      </section>
    </section>
    <section className="home-showcase" aria-labelledby="showcase-title">
      <div className="section-heading">
        <div><h2 id="showcase-title">From a piece to the whole interface.</h2><p>Keep composing. These controls share one Ubi application.</p></div>
        <Link href="/docs/components">Explore the library ↗</Link>
      </div>
      <NativePreview name="Ubi UI gallery" home code={gallery}/>
    </section>
    <section className="principles">
      <div>
        <span className="principle-icon">{`{ }`}</span><h2>Your components. Your source.</h2>
        <p>Import individual <code>.ubi</code> modules directly. Read the implementation, adapt it, and keep your interface in your language.</p>
        <Link href="/docs/installation">Start with source ↗</Link>
      </div>
      <div>
        <span className="principle-icon">⇄</span><h2>State that makes sense.</h2>
        <p>Controls emit typed events. Your reducer owns the model. A pure view function describes what appears next.</p>
        <Link href="/docs/state-and-events">Understand the model ↗</Link>
      </div>
      <div>
        <span className="principle-icon">◩</span><h2>One source. Multiple hosts.</h2>
        <p>The same Ubi source drives semantic HTML on the web, mobile WebViews, and a Rust desktop renderer.</p>
        <Link href="/docs/platforms">See platform status ↗</Link>
      </div>
    </section>
    <section className="home-start">
      <div><h2>From source to screen.</h2><p>Open the native gallery from the repository root.</p><Link className="text-link" href="/docs/installation">Installation and prerequisites ↗</Link></div>
      <Code language="terminal">{"cargo run --locked --features native-ui --\n  ui examples/gallery.ubi --root ubi-ui"}</Code>
    </section>
  </main>;
}
