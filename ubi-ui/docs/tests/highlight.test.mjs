import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { highlightUbi } from "../app/highlight.mjs";

function tokens(source) {
  const result = highlightUbi(source);
  assert.equal(result.map(token => token.text).join(""), source, "source must remain exact");
  assert.ok(result.every(token => token.text.length > 0));
  return result.filter(token => token.kind !== "plain");
}

test("highlights compiler keywords, primitives, types, calls, numbers and symbols", () => {
  const result = tokens("export fn greet(value: string) -> Window { return Window(12.5e-2, true); }");
  for (const [text, kind] of [["export", "keyword"], ["fn", "keyword"], ["greet", "function"], ["string", "type"], ["Window", "type"], ["12.5e-2", "number"], ["true", "keyword"], ["->", "operator"], ["{", "punctuation"]]) {
    assert.ok(result.some(token => token.text === text && token.kind === kind), `${text}: ${kind}`);
  }
});

test("keyword classification stays aligned with compiler lexer", () => {
  const lexer = readFileSync(new URL("../../../src/lexer.rs", import.meta.url), "utf8");
  const keywordBlock = lexer.match(/const KEYWORDS:[\s\S]*?= &\[([\s\S]*?)\];/)[1];
  for (const [, word] of keywordBlock.matchAll(/"([a-z]+)"/g)) {
    assert.ok(["keyword", "type"].includes(tokens(word)[0]?.kind), word);
  }
  assert.equal(tokens("imported internal_name").length, 0);
});

test("quoted comment markers and escaped quotes stay inside strings", () => {
  const source = String.raw`let label = "// café \" /* <b> */ \u{1F642}"; // "comment"`;
  const result = tokens(source);
  assert.equal(result.filter(token => token.kind === "string").length, 1);
  assert.equal(result.find(token => token.kind === "comment").text, '// "comment"');
});

test("block comments end at first terminator, including nested-looking content", () => {
  assert.deepEqual(tokens("/* outer /* inner */ return */").map(token => [token.kind, token.text]), [
    ["comment", "/* outer /* inner */"], ["keyword", "return"], ["operator", "*"], ["operator", "/"],
  ]);
});

test("comments, newlines, Unicode and HTML-like source retain exact text", () => {
  const source = '\t// <script>alert("x")</script> 🙂\r\nlet x = "árvíz & <img src=x onerror=alert(1)>";\r\n/* λ */\n';
  tokens(source);
  tokens("// no final newline");
  tokens("/* unfinished 🙂");
  tokens('"unfinished\\');
  tokens('"raw newline\nreturn 1');
  tokens("");
});

test("decimal exponents and punctuation follow Ubi numeric boundaries", () => {
  assert.deepEqual(tokens("1...2 -3 .5 1e+4 2E-2 4.0").filter(token => token.kind === "number").map(token => token.text), ["1", "2", "3", "5", "1e+4", "2E-2", "4.0"]);
});

test("all syntax colors maintain AA text contrast in both themes", () => {
  const css = readFileSync(new URL("../app/highlight.css", import.meta.url), "utf8");
  const styles = readFileSync(new URL("../app/style.css", import.meta.url), "utf8");
  const lightBackground = styles.match(/:root\{[^}]*--soft:(#[0-9a-f]+);/)[1];
  const darkBackground = styles.match(/:root\[data-theme=dark\]\{[^}]*--soft:(#[0-9a-f]+);/)[1];
  function luminance(hex) {
    let value = hex.slice(1);
    if (value.length === 3) value = [...value].map(digit => digit + digit).join("");
    const channels = value.match(/../g).map(part => parseInt(part, 16) / 255).map(part => part <= 0.04045 ? part / 12.92 : ((part + 0.055) / 1.055) ** 2.4);
    return channels[0] * 0.2126 + channels[1] * 0.7152 + channels[2] * 0.0722;
  }
  const colors = [...css.matchAll(/(^|\n)(:root\[data-theme=dark\] )?\.code-block \.ubi-token-([a-z]+)\{color:(#[0-9a-f]+)\}/g)];
  assert.equal(colors.length, 16);
  for (const [, , dark, kind, color] of colors) {
    const foreground = luminance(color);
    const background = luminance(dark ? darkBackground : lightBackground);
    const contrast = (Math.max(foreground, background) + 0.05) / (Math.min(foreground, background) + 0.05);
    assert.ok(contrast >= 4.5, `${dark ? "dark" : "light"} ${kind}: ${contrast}`);
  }
});
