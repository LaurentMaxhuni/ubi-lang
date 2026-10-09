import { refresh, clear } from "./.ubi-build/main.mjs";
import { createAsyncStorageCapability, createHttpCapability } from "./.ubi-build/.ubi-host.mjs";

const ok = value => JSON.stringify({ tag: "Ok", values: [value] });
const data = new Map();
const controller = new AbortController();
const storage = createAsyncStorageCapability({
  get: async key => ok(data.has(key) ? { tag: "Some", value: data.get(key) } : { tag: "None" }),
  set: async (key, value) => { data.set(key, value); return ok(null); },
  remove: async key => { data.delete(key); return ok(null); },
}, { signal: controller.signal });
// A deterministic trusted adapter: production adapters enforce URL authority and limits.
const http = createHttpCapability(async url => {
  if (url !== "https://example.test/message") {
    return JSON.stringify({ tag: "Err", values: [{ tag: "Failure", values: ["URL denied"] }] });
  }
  return ok("Sequential async Ubi");
}, { signal: controller.signal });

console.log(JSON.stringify(await refresh(storage, http)));
console.log(JSON.stringify(await clear(storage)));
controller.abort();
console.log(JSON.stringify(await refresh(storage, http)));
