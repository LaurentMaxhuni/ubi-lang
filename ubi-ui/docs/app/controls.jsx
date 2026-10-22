"use client";
import { useEffect, useId, useRef, useState } from "react";
import Link from "next/link";
import { usePathname } from "next/navigation";
import { highlightUbi } from "./highlight.mjs";
export { NativePreview } from "./preview";
function SearchIcon() { return <svg viewBox="0 0 20 20" fill="none" aria-hidden="true"><circle cx="8" cy="8" r="5.5" stroke="currentColor" strokeWidth="1.5"/><path d="m12 12 5 5" stroke="currentColor" strokeWidth="1.5"/></svg>; }
export function Mark() { return <span className="mark" aria-hidden="true"><svg viewBox="0 0 24 24" fill="none"><path d="M5 5v9a7 7 0 0 0 14 0V5M9 5v9a3 3 0 0 0 6 0V5" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"/></svg></span>; }
function Navigation({ guides, components, pathname, onNavigate }) {
  const groups = [...new Set(components.map(item => item.category))];
  return <nav className="side-nav" aria-label="Documentation"><div className="nav-group"><h2>Getting started</h2>{guides.map(item => <Link aria-current={pathname === item.href ? "page" : undefined} onClick={onNavigate} key={item.href} href={item.href}>{item.title}</Link>)}</div><div className="nav-group"><h2>Components</h2><Link href="/docs/components" onClick={onNavigate} aria-current={pathname === "/docs/components" ? "page" : undefined}>All components <span className="nav-count">{components.length}</span></Link></div>{groups.map(group => <div className="nav-group" key={group}><h2>{group}</h2>{components.filter(item => item.category === group).map(item => <Link aria-current={pathname === `/docs/components/${item.slug}` ? "page" : undefined} onClick={onNavigate} key={item.slug} href={`/docs/components/${item.slug}`}>{item.name}</Link>)}</div>)}<p className="sidebar-foot">Built in Rust.<br/>Declared in Ubi.</p></nav>;
}
export function SiteShell({ children, guides, components, searchEntries }) {
  const pathname = usePathname();
  const [theme, setTheme] = useState("light");
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const search = useRef(null), drawer = useRef(null), input = useRef(null), resultsRef = useRef(null);
  const searchId = useId();
  const results = searchEntries.filter(entry => `${entry.title} ${entry.description} ${entry.group}`.toLowerCase().includes(query.trim().toLowerCase()));
  function openSearch() { setQuery(""); setActive(0); search.current.showModal(); input.current?.focus(); }
  useEffect(() => {
    setTheme(document.documentElement.dataset.theme || "light");
    function onKey(event) { if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") { event.preventDefault(); if (search.current.open) search.current.close(); else openSearch(); } }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
  function toggleTheme() { const next = theme === "dark" ? "light" : "dark"; setTheme(next); document.documentElement.dataset.theme = next; try { localStorage.setItem("ubi-theme", next); } catch {} }
  function searchKeys(event) {
    if (event.key === "ArrowDown" || event.key === "ArrowUp") { event.preventDefault(); const next = Math.max(0, Math.min(results.length - 1, active + (event.key === "ArrowDown" ? 1 : -1))); setActive(next); resultsRef.current?.children[next]?.scrollIntoView({ block: "nearest" }); }
    if (event.key === "Enter" && results.length) { event.preventDefault(); resultsRef.current?.children[active]?.click(); }
  }
  return <>
    <a className="skip" href="#content">Skip to content</a>
    <header className="site-header"><div className="header-inner"><button className="icon-button mobile-menu" aria-label="Open navigation" onClick={() => drawer.current.showModal()}><svg viewBox="0 0 24 24" fill="none" aria-hidden="true"><path d="M4 6h16M4 12h16M4 18h16" stroke="currentColor" strokeWidth="1.5"/></svg></button><Link className="brand" href="/" aria-label="Ubi UI home"><Mark/><span>ubi<span className="brand-slash">/</span>ui</span></Link><nav className="top-nav" aria-label="Main navigation"><Link href="/docs" aria-current={pathname.startsWith("/docs") && !pathname.startsWith("/docs/components") ? "page" : undefined}>Docs</Link><Link href="/docs/components" aria-current={pathname.startsWith("/docs/components") ? "page" : undefined}>Components</Link></nav><div className="header-actions"><button className="search-trigger" aria-label="Search documentation" aria-keyshortcuts="Control+K Meta+K" onClick={openSearch}><SearchIcon/><span>Search documentation…</span><kbd>⌘ K</kbd></button><a className="repository-link" href="https://github.com/LaurentMaxhuni/ubi-lang" aria-label="Ubi on GitHub"><svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M12 2a10 10 0 0 0-3.16 19.49c.5.09.68-.22.68-.48v-1.86c-2.78.6-3.37-1.18-3.37-1.18-.45-1.16-1.11-1.47-1.11-1.47-.91-.62.07-.61.07-.61 1 .07 1.53 1.03 1.53 1.03.89 1.53 2.34 1.09 2.91.83.09-.65.35-1.09.63-1.34-2.22-.25-4.56-1.11-4.56-4.94 0-1.09.39-1.99 1.03-2.69-.1-.25-.45-1.27.1-2.65 0 0 .84-.27 2.75 1.03A9.6 9.6 0 0 1 12 6.82c.85 0 1.71.11 2.51.34 1.91-1.3 2.75-1.03 2.75-1.03.55 1.38.2 2.4.1 2.65.64.7 1.03 1.6 1.03 2.69 0 3.84-2.34 4.69-4.57 4.94.36.31.68.92.68 1.85v2.75c0 .27.18.58.69.48A10 10 0 0 0 12 2Z"/></svg></a><span className="header-divider"/><button className="icon-button theme-button" aria-label={`Switch to ${theme === "dark" ? "light" : "dark"} theme`} onClick={toggleTheme}><svg viewBox="0 0 24 24" fill="none" aria-hidden="true">{theme === "dark" ? <path d="M20 14.5A8.5 8.5 0 0 1 9.5 4 8.5 8.5 0 1 0 20 14.5Z" stroke="currentColor" strokeWidth="1.5"/> : <><circle cx="12" cy="12" r="4" stroke="currentColor" strokeWidth="1.5"/><path d="M12 2v2m0 16v2M2 12h2m16 0h2M5 5l1.5 1.5m11 11L19 19M5 19l1.5-1.5m11-11L19 5" stroke="currentColor" strokeWidth="1.5"/></>}</svg></button></div></div></header>
    {pathname.startsWith("/docs") ? <div className="docs-shell"><aside className="desktop-sidebar"><Navigation guides={guides} components={components} pathname={pathname}/></aside>{children}</div> : children}
    <footer className="site-footer"><span>Ubi UI</span><p>Source-owned Ubi components for web, mobile and desktop.</p><Link href="/docs/platforms">Platform status</Link></footer>
    <dialog className="search-dialog" ref={search} aria-labelledby={`${searchId}-title`} onClick={event => { if (event.target === event.currentTarget) search.current.close(); }}><div className="search-dialog-inner"><div className="search-input-row"><label className="sr-only" id={`${searchId}-title`} htmlFor={`${searchId}-input`}>Search documentation</label><SearchIcon/><input ref={input} id={`${searchId}-input`} value={query} onChange={event => { setQuery(event.target.value); setActive(0); }} onKeyDown={searchKeys} placeholder="Search components and guides…" autoComplete="off"/><button className="close-button" onClick={() => search.current.close()} aria-label="Close search">Esc</button></div><div className="search-results" ref={resultsRef}>{results.map((entry, index) => <Link className={index === active ? "search-result selected" : "search-result"} href={entry.href} key={entry.href} onClick={() => search.current.close()}><span><strong>{entry.title}</strong><small>{entry.description}</small></span><span className="result-group">{entry.group}</span></Link>)}</div>{!results.length && <p className="search-empty">No results for “{query}”. Try a component name or guide.</p>}<p className="sr-only" role="status">{results.length} results. {results[active] ? `Selected: ${results[active].title}` : ""}</p><div className="search-hint"><span><kbd>↑</kbd> <kbd>↓</kbd> to navigate</span><span><kbd>↵</kbd> to open</span><span><kbd>Esc</kbd> to close</span></div></div></dialog>
    <dialog className="mobile-drawer" ref={drawer} aria-label="Navigation"><div className="drawer-head"><Link href="/" className="brand" onClick={() => drawer.current.close()}><Mark/>ubi/ui</Link><button className="icon-button" onClick={() => drawer.current.close()} aria-label="Close navigation">✕</button></div><Navigation guides={guides} components={components} pathname={pathname} onNavigate={() => drawer.current.close()}/></dialog>
  </>;
}
export function Code({ children, language = "ubi", title }) {
  const [feedback, setFeedback] = useState("");
  const timer = useRef(null), code = useRef(null);
  useEffect(() => () => clearTimeout(timer.current), []);
  async function copy() { try { await navigator.clipboard.writeText(children); setFeedback("Copied"); } catch { const selection = window.getSelection(), range = document.createRange(); range.selectNodeContents(code.current); selection.removeAllRanges(); selection.addRange(range); setFeedback("Select & copy"); } clearTimeout(timer.current); timer.current = setTimeout(() => setFeedback(""), 2500); }
  const highlighted = language === "ubi" && typeof children === "string" ? highlightUbi(children) : null;
  return <div className="code-block"><div className="code-toolbar"><span>{title || language}</span><button onClick={copy} aria-label={`Copy ${title || language} code`}>{feedback || "Copy"}</button><span className="sr-only" role="status">{feedback}</span></div><pre tabIndex="0"><code ref={code} className={`language-${language}`}>{highlighted ? highlighted.map((token, index) => token.kind === "plain" ? token.text : <span key={index} className={`ubi-token-${token.kind}`}>{token.text}</span>) : children}</code></pre></div>;
}
