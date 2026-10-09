const capabilities = new WeakMap();

function register(kind, operations, signal) {
  const token = Object.freeze(Object.create(null));
  capabilities.set(token, { kind, operations, signal });
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

export function createAsyncStorageCapability(adapter, { signal } = {}) {
  const { get, set, remove } = adapter;
  if ([get, set, remove].some(callback => typeof callback !== "function")) {
    throw new TypeError("Async storage adapter requires get, set, remove callbacks");
  }
  return register("storage", { storageGetAsync: get, storageSetAsync: set, storageRemoveAsync: remove }, signal);
}

export function createHttpCapability(get, { signal } = {}) {
  if (typeof get !== "function") throw new TypeError("HTTP adapter requires get callback");
  return register("http", { httpGet: get }, signal);
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

export async function callHostAsync(name, args, decode, makeEnum) {
  const capability = capabilities.get(args[0]);
  const callback = capability?.operations[name];
  if (!callback) throw new TypeError("Invalid Ubi capability");
  const signal = capability.signal;
  const cancelled = () => makeEnum("Err", [makeEnum("Cancelled", ["Host operation cancelled"])]);
  if (signal?.aborted) return cancelled();
  let abort;
  try {
    const pending = new Promise((resolve, reject) => {
      let completed = false;
      const finish = (action, value) => {
        if (completed) return;
        completed = true;
        action(value);
      };
      abort = () => finish(resolve, { cancelled: true });
      signal?.addEventListener("abort", abort, { once: true });
      if (signal?.aborted) { abort(); return; }
      let response;
      try {
        response = callback(...args.slice(1), signal);
        if (!response || typeof response.then !== "function") throw new TypeError("Async adapter must return a Promise");
        Promise.resolve(response).then(
          value => finish(resolve, { value }),
          () => finish(reject, Object.freeze({ code: "UBI-R0004", message: "Unexpected trusted adapter fault" }))
        );
      } catch {
        finish(reject, Object.freeze({ code: "UBI-R0004", message: "Unexpected trusted adapter fault" }));
      }
    });
    const response = await pending;
    if (response.cancelled) return cancelled();
    try {
      return decode(response.value);
    } catch {
      return makeEnum("Err", [makeEnum("Decode", ["Malformed host response"])]);
    }
  } finally {
    if (abort) signal?.removeEventListener("abort", abort);
  }
}
