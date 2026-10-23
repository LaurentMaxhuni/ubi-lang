import Link from "next/link";
import { Code, NativePreview } from "./controls";
import { Section } from "./document";
import { readSource } from "./lib/content";

const examples = [
  ["number-ticker", "Number Ticker", "Change a value and watch its presentation settle. Your reducer owns the exact number."],
  ["text-reveal", "Text Reveal", "Let words arrive together with a measured rhythm. The complete sentence remains accessible."],
  ["animated-list", "Animated List", "Reorder real keyed Ubi views while their identity and focus stay intact."],
  ["shimmer-button", "Shimmer Button", "A moving highlight with ordinary button semantics, typed actions, and disabled states."],
];

export default function MotionGuide() {
  return <>
    <Section id="motion-contract" title="One model. A little movement.">
      <p>Write components and reducers in Ubi. The host animates presentation between committed values, keeping application state independent of frame timing.</p>
      <Code>{'import { Motion, Easing, Transition, Prop } from "../../packages/ubi-ui/core.ubi";\n\n// Properties for a keyed component\n[\n    Prop.Motion(Motion { durationMs: 220, delayMs: 0, easing: Easing.Spring }),\n    Prop.Enter(Transition.Rise)\n]'}</Code>
      <p>Use stable keys when values change. A second update starts from the currently displayed position. Set <code>durationMs</code> to <code>0</code> for immediate presentation.</p>
      <p>Durations accept 0–2000 milliseconds and delays 0–1000 milliseconds. Linear, EaseOut, EaseInOut, and Spring easing share the same bounded sampler on both hosts.</p>
    </Section>
    <Section id="motion-examples" title="Try the components">
      <p>These previews run compiled Ubi source. Change a value, reorder content, or replay an entrance; open the Code tab to see the complete application.</p>
      {examples.map(([slug, name, description]) => <div className="motion-example" key={slug}>
        <h3><Link href={`/docs/components/${slug}`}>{name} ↗</Link></h3>
        <p>{description}</p>
        <NativePreview slug={slug} name={name} code={readSource(`examples/components/${slug}.ubi`)}/>
      </div>)}
    </Section>
    <Section id="motion-preferences" title="Movement is a preference">
      <p>The browser follows your system’s reduced-motion preference. Motion never gates essential content, and decorative loops settle to a static appearance when movement is reduced.</p>
      <Code language="javascript">{'const handle = mount(root, app, { motion: "system" });\nhandle.setMotion("reduced");\n// Restore the system preference\nhandle.setMotion("system");'}</Code>
      <p>The native host defaults to the system preference. Embedders can select <code>MotionPreference::Full</code> or <code>MotionPreference::Reduced</code> with <code>run_with_motion</code>. Unsupported system adapters use reduced motion.</p>
      <p>Pause controls belong alongside automatically moving content. Focus and selection update immediately, even while a visual transition is finishing.</p>
    </Section>
    <Section id="motion-support" title="Shared behavior, visible boundaries">
      <p>The Ubi model and typed component API are shared by the browser and Rust desktop hosts. Some decorative effects use a static native treatment; each component documents its visual support.</p>
      <p>Native screenshots show a sampled frame. Use the live preview to inspect browser motion and run the source application to check your own native environment. See <Link href="/docs/platforms">platform status</Link> for current verification boundaries.</p>
    </Section>
  </>;
}
