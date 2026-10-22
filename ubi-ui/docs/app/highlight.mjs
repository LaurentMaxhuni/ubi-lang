// Lexical boundaries mirror src/lexer.rs. Incomplete examples remain readable;
// highlighting deliberately does not validate or transform source text.
const keywords = new Set("import from export fn record enum let mut return if else match true false async await throw try catch class while for in break continue new null undefined as".split(" "));
const primitives = new Set("int float bool string unit any".split(" "));
const word = /[A-Za-z_][A-Za-z_0-9]*/y;
const number = /[0-9]+(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?/y;
const symbol = /\.\.\.|->|=>|==|!=|<=|>=|&&|\|\||[()[\]{},:;._+*/%!=<>?\-]/y;

export function highlightUbi(source) {
  const tokens = [];
  let offset = 0;
  function push(kind, end) {
    const text = source.slice(offset, end);
    const previous = tokens.at(-1);
    if (kind === "plain" && previous?.kind === kind) previous.text += text;
    else tokens.push({ kind, text });
    offset = end;
  }
  while (offset < source.length) {
    if (source.startsWith("//", offset)) {
      let end = offset + 2;
      while (end < source.length && !/[\r\n]/.test(source[end])) end++;
      push("comment", end);
    } else if (source.startsWith("/*", offset)) {
      // Ubi block comments do not nest.
      const end = source.indexOf("*/", offset + 2);
      push("comment", end < 0 ? source.length : end + 2);
    } else if (source[offset] === '"') {
      let end = offset + 1;
      while (end < source.length && !/[\r\n]/.test(source[end])) {
        const current = source[end++];
        if (current === '"') break;
        if (current === "\\" && end < source.length && !/[\r\n]/.test(source[end])) end++;
      }
      push("string", end);
    } else {
      word.lastIndex = number.lastIndex = symbol.lastIndex = offset;
      let match;
      if ((match = word.exec(source))) {
        const text = match[0];
        const end = offset + text.length;
        const kind = primitives.has(text) || /^[A-Z]/.test(text) ? "type"
          : keywords.has(text) ? "keyword"
          : text === "_" ? "punctuation"
          : /^[\t\n\r ]*\(/.test(source.slice(end)) ? "function" : "plain";
        push(kind, end);
      } else if ((match = number.exec(source))) {
        push("number", offset + match[0].length);
      } else if ((match = symbol.exec(source))) {
        push(/^[()[\]{},:;._]/.test(match[0]) ? "punctuation" : "operator", offset + match[0].length);
      } else {
        push("plain", offset + (source.codePointAt(offset) > 0xffff ? 2 : 1));
      }
    }
  }
  return tokens;
}
