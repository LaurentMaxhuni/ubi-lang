import { main } from "./.ubi-build/main.mjs";
import { createConsoleCapability, createStorageCapability } from "./.ubi-build/.ubi-host.mjs";

const ok = value => JSON.stringify({ tag: "Ok", values: [value] });
const values = new Map();
const consoleCapability = createConsoleCapability(message => {
  console.log(message);
  return ok(null);
});
const storageCapability = createStorageCapability({
  get: key => ok(values.has(key) ? { tag: "Some", value: values.get(key) } : { tag: "None" }),
  set: (key, value) => { values.set(key, value); return ok(null); },
  remove: key => { values.delete(key); return ok(null); },
});

console.log(JSON.stringify(main(consoleCapability, storageCapability)));
