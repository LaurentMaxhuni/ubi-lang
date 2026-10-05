const capabilities = new WeakMap();

function register(kind, operations) {
  const token = Object.freeze(Object.create(null));
  capabilities.set(token, { kind, operations });
  return token;
}

export function createConsoleCapability(write) {
  if (typeof write !== "function") throw new TypeError("Console adapter requires write callback");
  return register("console", { consoleLog: write });
}

export function createStorageCapability(adapter) {
  const { get, set, remove } = adapter;
  if ([get, set, remove].some(callback => typeof callback !== "function")) {
    throw new TypeError("Storage adapter requires get, set, remove callbacks");
  }
  return register("storage", { storageGet: get, storageSet: set, storageRemove: remove });
}

export function isCapability(value, kind) {
  return capabilities.get(value)?.kind === kind;
}

export function callHost(name, args, decode, makeEnum) {
  const capability = capabilities.get(args[0]);
  const callback = capability?.operations[name];
  if (!callback) throw new TypeError("Invalid Ubi capability");
  let response;
  try {
    response = callback(...args.slice(1));
  } catch {
    throw Object.freeze({ code: "UBI-R0004", message: "Unexpected trusted adapter fault" });
  }
  try {
    return decode(response);
  } catch {
    return makeEnum("Err", [makeEnum("Decode", ["Malformed host response"])]);
  }
}
