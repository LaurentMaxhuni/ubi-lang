"use client";
import { useState } from "react";
export default function NativeGallery() {
  const [capture, setCapture] = useState("light");
  return <div className="gallery">
    <fieldset><legend>Native capture</legend><div className="capture-choices">{["light", "dark", "narrow"].map(name => <label key={name}><input type="radio" name="capture" value={name} checked={capture === name} onChange={() => setCapture(name)} /><span>{name[0].toUpperCase() + name.slice(1)}</span></label>)}</div></fieldset>
    <figure><img src={`/native-${capture}.png`} width={capture === "narrow" ? 420 : 1100} height="1000" alt={`${capture} appearance of the real native Ubi gallery`} /><figcaption>Verified native renderer output · {capture === "narrow" ? "420 × 1000 logical pixels" : "1100 × 1000 logical pixels"}</figcaption></figure>
  </div>;
}
