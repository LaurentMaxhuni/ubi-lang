import fs from "node:fs";
import path from "node:path";
const root = path.resolve(process.cwd(), "..");
export const readSource = relative => fs.readFileSync(path.join(root, relative), "utf8").trim();
export const components = JSON.parse(readSource("catalog.json"));
export const guides = [
  { slug: "", title: "Introduction", description: "Native interfaces. Ubi source. Your application." },
  { slug: "installation", title: "Installation", description: "Run a native Ubi application from this repository." },
  { slug: "state-and-events", title: "State & events", description: "A model, a reducer, and a view. One predictable update cycle." },
  { slug: "layout", title: "Layout", description: "Compose stacks, flexible rows, grids, and scrolling content." },
  { slug: "theming", title: "Theming", description: "Consistent surfaces, spacing, and control states in light and dark." },
  { slug: "motion", title: "Motion", description: "Expressive interactions, shared Ubi state, and motion that respects your preferences." },
  { slug: "accessibility", title: "Accessibility", description: "Build clear names, visible focus, and keyboard behavior into your interface." },
  { slug: "platforms", title: "Platforms", description: "Current host support, verification boundaries, and known limitations." },
];
export const guideHref = guide => `/docs${guide.slug ? `/${guide.slug}` : ""}`;
export function componentSource(component) {
  const source = readSource(component.module);
  const signature = source.split("\n").find(line => line.startsWith(`export fn ${component.function}<`) || line.startsWith(`export fn ${component.function}(`))?.split(" {")[0];
  if (!signature) throw new Error(`Missing public declaration for ${component.slug}`);
  return { source, signature, examples: component.examples.map(example => ({ ...example, code: readSource(example.source) })) };
}
export const searchEntries = [...guides.map(guide => ({ title: guide.title, description: guide.description, href: guideHref(guide), group: "Documentation" })), ...components.map(component => ({ title: component.name, description: component.description, href: `/docs/components/${component.slug}`, group: component.category }))];
