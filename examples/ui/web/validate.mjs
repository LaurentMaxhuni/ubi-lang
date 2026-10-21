import { encode } from './codec.mjs';

const layouts = new Set(['Column', 'Row', 'Card']);
const kinds = new Set([...layouts, 'Text', 'Heading', 'Badge', 'Button']);
const choices = {
  Gap: ['None', 'Xs', 'Sm', 'Md', 'Lg', 'Xl', 'Xxl'],
  Tone: ['Neutral', 'Primary', 'Success', 'Warning', 'Danger'],
  Variant: ['Primary', 'Secondary', 'Outline', 'Ghost', 'Destructive'],
  Theme: ['Light', 'Dark', 'System'],
};
const fail = message => { throw new TypeError(`Invalid Ubi UI view: ${message}`); };
function record(value, keys) {
  if (!value || Array.isArray(value) || typeof value !== 'object' ||
      Object.keys(value).sort().join(',') !== [...keys].sort().join(',')) fail('record fields');
}
function variant(value, arity) {
  record(value, ['tag', 'values']);
  if (typeof value.tag !== 'string' || !Array.isArray(value.values) || value.values.length !== arity) fail('variant shape');
  return value.tag;
}
function freeze(value) {
  if (value && typeof value === 'object' && !Object.isFrozen(value)) {
    for (const child of Object.values(value)) freeze(child);
    Object.freeze(value);
  }
  return value;
}
export function validateView(value) {
  // Normalize into owned data before inspecting it. This checks bytes/depth,
  // rejects accessors/cycles and prevents caller mutation after validation.
  const view = JSON.parse(encode(value));
  record(view, ['schemaVersion', 'root', 'nodes']);
  if (view.schemaVersion !== 1) fail('schema version');
  if (!Array.isArray(view.nodes) || view.nodes.length < 1 || view.nodes.length > 4096) fail('node count');
  const byKey = new Map();
  for (const source of view.nodes) {
    record(source, ['key', 'kind', 'props', 'children']);
    if (typeof source.key !== 'string' || [...source.key].length < 1 || [...source.key].length > 128) fail('key length');
    if (byKey.has(source.key)) fail('duplicate key');
    const kind = variant(source.kind, 0);
    if (!kinds.has(kind)) fail('unknown kind');
    if (!Array.isArray(source.props) || !Array.isArray(source.children) || source.children.some(key => typeof key !== 'string')) fail('props/children');
    const props = Object.create(null);
    for (const prop of source.props) {
      const name = variant(prop, 1), data = prop.values[0];
      if (Object.hasOwn(props, name)) fail('duplicate prop');
      const allowed = layouts.has(kind) ? ['Gap'] : kind === 'Button' ? ['Content', 'Variant', 'Disabled', 'Busy', 'Activate', 'Pressed'] : kind === 'Badge' ? ['Content', 'Tone'] : kind === 'Heading' ? ['Content', 'Level'] : ['Content'];
      if (source.key === view.root && layouts.has(kind)) allowed.push('Theme');
      if (!allowed.includes(name)) fail('prop not allowed for kind');
      if (choices[name]) {
        if (!choices[name].includes(variant(data, 0))) fail('prop choice');
        props[name] = data.tag;
      } else {
        if (name === 'Content' && typeof data !== 'string') fail('content');
        if (name === 'Level' && (!Number.isInteger(data) || data < 1 || data > 6)) fail('heading level');
        if (['Disabled', 'Busy', 'Pressed'].includes(name) && typeof data !== 'boolean') fail('boolean prop');
        props[name] = data;
      }
    }
    const required = layouts.has(kind) ? [] : kind === 'Button' ? ['Content', 'Variant', 'Disabled', 'Busy', 'Activate'] : kind === 'Badge' ? ['Content', 'Tone'] : kind === 'Heading' ? ['Content', 'Level'] : ['Content'];
    if (required.some(name => !Object.hasOwn(props, name))) fail('missing required prop');
    if (kind === 'Button' && props.Content.trim().length === 0) fail('button label');
    if (!layouts.has(kind) && source.children.length) fail('leaf children / nested interactive controls');
    byKey.set(source.key, { key: source.key, kind, props, children: source.children });
  }
  if (!byKey.has(view.root)) fail('root');
  const seen = new Set(), preorder = [], stack = [[view.root, 1]];
  while (stack.length) {
    const [key, depth] = stack.pop(), node = byKey.get(key);
    if (!node) fail('missing child');
    if (seen.has(key)) fail('cycle or multiple parents');
    if (depth > 64) fail('graph depth');
    seen.add(key); preorder.push(node);
    for (let i = node.children.length - 1; i >= 0; i--) stack.push([node.children[i], depth + 1]);
  }
  if (seen.size !== byKey.size) fail('unreachable nodes');
  return freeze({ root: view.root, nodes: preorder });
}
