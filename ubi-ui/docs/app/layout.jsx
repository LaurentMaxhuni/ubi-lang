import "./style.css";
import { SiteShell } from "./controls";
import { guides, guideHref, components, searchEntries } from "./lib/content";
export const metadata = { title: { default: "Ubi UI — Build native interfaces with Ubi", template: "%s — Ubi UI" }, description: "Source-owned Ubi components. A Rust native runtime. Build interfaces with predictable state and real GPU rendering." };
export default function Layout({ children }) {
  return <html lang="en" suppressHydrationWarning><head><script dangerouslySetInnerHTML={{ __html: `try{document.documentElement.dataset.theme=localStorage.getItem('ubi-theme')||(matchMedia('(prefers-color-scheme: dark)').matches?'dark':'light')}catch{document.documentElement.dataset.theme='light'}` }}/></head><body><SiteShell guides={guides.map(guide => ({ ...guide, href: guideHref(guide) }))} components={components.map(({ slug, name, category }) => ({ slug, name, category }))} searchEntries={searchEntries}>{children}</SiteShell></body></html>;
}
