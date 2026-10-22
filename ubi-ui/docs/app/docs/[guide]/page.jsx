import { notFound } from "next/navigation";
import { Document } from "../../document";
import GuideContent, { guideOutlines } from "../../guide-content";
import { guides, guideHref } from "../../lib/content";
export const dynamicParams = false;
export function generateStaticParams() { return guides.filter(guide => guide.slug).map(guide => ({ guide: guide.slug })); }
export async function generateMetadata({ params }) { const { guide } = await params; return { title: guides.find(item => item.slug === guide)?.title }; }
export default async function GuidePage({ params }) { const { guide } = await params; const index = guides.findIndex(item => item.slug === guide); if (index < 1) notFound(); const previous = guides[index - 1], next = guides[index + 1]; return <Document {...guides[index]} outline={guideOutlines[guide]} previous={{ title: previous.title, href: guideHref(previous) }} next={next ? { title: next.title, href: guideHref(next) } : { title: "Components", href: "/docs/components" }}><GuideContent slug={guide}/></Document>; }
