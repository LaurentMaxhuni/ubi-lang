// The public Ubi ABI accepts aggregates as JSON text, including tagged floats.
const MAX_BYTES = 1_048_576;
export function encode(value) {
  let bytes = 0, buffer = '';
  const chunks = [], active = new Set();
  function write(text, size = text.length) {
    bytes += size;
    if (bytes > MAX_BYTES) throw new TypeError('Ubi UI payload byte budget exceeded');
    buffer += text;
    if (buffer.length >= 4096) { chunks.push(buffer); buffer = ''; }
  }
  function string(text) {
    write('"');
    for (let i = 0; i < text.length; i++) {
      const code = text.codePointAt(i);
      if (code >= 0xd800 && code <= 0xdfff) throw new TypeError('Invalid Unicode scalar');
      const char = String.fromCodePoint(code);
      if (code > 0xffff) i++;
      if (code < 32 || char === '"' || char === '\\') write(JSON.stringify(char).slice(1, -1));
      else write(char, code < 128 ? 1 : code < 2048 ? 2 : code < 65536 ? 3 : 4);
    }
    write('"');
  }
  function visit(item, depth) {
    if (item == null) { write('null'); return; }
    if (typeof item === 'string') { string(item); return; }
    if (typeof item === 'boolean') { write(String(item)); return; }
    if (typeof item === 'number') {
      if (Number.isNaN(item)) string('NaN');
      else if (item === Infinity) string('+Infinity');
      else if (item === -Infinity) string('-Infinity');
      else if (Object.is(item, -0)) string('-0');
      else write(String(item));
      return;
    }
    if (typeof item !== 'object') throw new TypeError('Unsupported Ubi UI value');
    if (active.has(item)) throw new TypeError('Cyclic Ubi UI value');
    if (depth >= 32) throw new TypeError('Ubi UI aggregate depth exceeded');
    const array = Array.isArray(item), proto = Object.getPrototypeOf(item);
    if (proto !== (array ? Array.prototype : Object.prototype) && !(proto === null && !array)) {
      throw new TypeError('Ubi UI requires plain own-data records');
    }
    const fields = Object.getOwnPropertyDescriptors(item);
    for (const key of Reflect.ownKeys(fields)) {
      const field = fields[key];
      if (!Object.hasOwn(field, 'value') || (typeof key === 'symbol' && field.enumerable)) {
        throw new TypeError('Ubi UI requires own data properties');
      }
    }
    active.add(item);
    if (array) {
      write('[');
      for (let i = 0; i < fields.length.value; i++) {
        if (i) write(',');
        if (!fields[i]) throw new TypeError('Sparse arrays are not Ubi values');
        visit(fields[i].value, depth + 1);
      }
      write(']');
    } else {
      write('{');
      let first = true;
      for (const key of Object.keys(fields)) {
        if (!fields[key].enumerable) continue;
        if (!first) write(',');
        first = false;
        string(key); write(':'); visit(fields[key].value, depth + 1);
      }
      write('}');
    }
    active.delete(item);
  }
  visit(value, 0);
  chunks.push(buffer);
  return chunks.join('');
}
