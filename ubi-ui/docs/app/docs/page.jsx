import { Document } from "../document";
import GuideContent, { guideOutlines } from "../guide-content";
import { guides, guideHref } from "../lib/content";
export const metadata = { title: "Introduction" };
export default function Introduction() { return <Document {...guides[0]} outline={guideOutlines[""]} next={{ title: guides[1].title, href: guideHref(guides[1]) }}><GuideContent slug=""/></Document>; }
