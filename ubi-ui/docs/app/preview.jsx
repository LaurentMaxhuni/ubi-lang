"use client";
import { useEffect, useId, useRef, useState } from "react";
import { Code } from "./controls";

export function NativePreview({ slug, name, code, home = false }) {
  const [tab, setTab] = useState("preview");
  const [version, setVersion] = useState(0);
  const [status, setStatus] = useState("loading");
  const [error, setError] = useState("");
  const [motion, setMotion] = useState("system");
  const frame = useRef(null);
  const id = useId();
  const example = home ? "gallery" : slug;
  const tabs = ["preview", "native", "code"];
  function sendTheme() { frame.current?.contentWindow?.postMessage({ type: "ubi-preview-theme", theme: document.documentElement.dataset.theme || "system" }, location.origin); frame.current?.contentWindow?.postMessage({type:"ubi-preview-motion",motion},location.origin); }
  function syncFrame() {
    const document = frame.current?.contentDocument;
    if (document?.documentElement.dataset.ubiStatus === "ready") { setStatus("ready"); sendTheme(); }
    if (document?.documentElement.dataset.ubiStatus === "error") { setStatus("error"); setError(document.getElementById("error")?.textContent || "Example failed to load"); }
  }
  useEffect(() => {
    function receive(event) {
      if (event.source !== frame.current?.contentWindow || event.origin !== location.origin || event.data?.type !== "ubi-preview-status") return;
      if (event.data.status === "ready") { setStatus("ready"); sendTheme(); }
      if (event.data.status === "error") { setStatus("error"); setError(String(event.data.message)); }
    }
    const observer = new MutationObserver(sendTheme);
    observer.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
    window.addEventListener("message", receive);
    syncFrame();
    return () => { observer.disconnect(); window.removeEventListener("message", receive); };
  }, [version, motion]);
  return <><div className="workbench-heading">{home ? "Build, change, repeat." : "Component workbench · try it here"}</div><div className={`native-preview${home ? " home-preview" : ""}`}>
    <div className="preview-toolbar"><div role="tablist" aria-label={`${name} example`} onKeyDown={event => {
      if (!["ArrowRight", "ArrowLeft", "Home", "End"].includes(event.key)) return;
      event.preventDefault();
      const index = tabs.indexOf(tab);
      const next = tabs[event.key === "Home" ? 0 : event.key === "End" ? tabs.length - 1 : (index + (event.key === "ArrowRight" ? 1 : -1) + tabs.length) % tabs.length];
      setTab(next); document.getElementById(`${id}-${next}`)?.focus();
    }}>{tabs.map(value => <button key={value} id={`${id}-${value}`} role="tab" tabIndex={tab === value ? 0 : -1} aria-selected={tab === value} aria-controls={`${id}-${value}-panel`} onClick={() => setTab(value)}>{value === "preview" ? "Preview" : value === "native" ? "Native" : "Code"}</button>)}</div><div className="preview-actions">{tab === "preview" && <button className="preview-reset" aria-label={`Reset ${name} example`} onClick={() => { setStatus("loading"); setError(""); setVersion(value => value + 1); }}>Reset</button>}<span className="native-label"><span/>{tab === "native" ? "Native capture" : "Live web"}</span></div></div>
    <div id={`${id}-preview-panel`} role="tabpanel" aria-labelledby={`${id}-preview`} hidden={tab !== "preview"}>
      <div className="preview-preferences"><label>Motion <select aria-label={`${name} preview motion`} value={motion} onChange={event=>setMotion(event.target.value)}><option value="system">System preference</option><option value="full">Full motion</option><option value="reduced">Reduced motion</option></select></label></div>
      {status === "loading" && <p className="live-status" role="status">Loading interactive Ubi example…</p>}
      <div className="live-stage"><iframe key={version} ref={frame} title={`${name} live web example`} src={`/previews/${example}/index.html?preview=true`} style={{height:slug==='text-reveal'?'540px':slug==='popover'?'620px':undefined}} loading={home ? "eager" : "lazy"} onLoad={syncFrame}/></div>
      {status === "error" && <p className="live-error" role="alert">Unable to run this example: {error}</p>}
      <div className="capture-caption">Interactive Ubi preview<a href={`/previews/${example}/index.html`} target="_blank" rel="noreferrer">Open example ↗</a></div>
    </div>
    <div id={`${id}-native-panel`} role="tabpanel" aria-labelledby={`${id}-native`} hidden={tab !== "native"}><div className="capture-stage"><img className="light-capture" src={home ? "/native-light.png" : `/components/${slug}-light.png`} alt={`${name}: native renderer capture in light appearance`} loading="lazy"/><img className="dark-capture" src={home ? "/native-dark.png" : `/components/${slug}-dark.png`} alt={`${name}: native renderer capture in dark appearance`} loading="lazy"/></div><div className="capture-caption">Native desktop capture<span>Rendered by Ubi + wgpu</span></div></div>
    <div id={`${id}-code-panel`} role="tabpanel" aria-labelledby={`${id}-code`} hidden={tab !== "code"}><Code title="Ubi example">{code}</Code></div>
  </div></>;
}
