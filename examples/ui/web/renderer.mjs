import { encode } from './codec.mjs';
import { validateView } from './validate.mjs';

const owners = new WeakMap();
let nextMount = 0;
const result = value => {
  if (!value || value.tag !== 'Ok' || !Array.isArray(value.values) || value.values.length !== 1) {
    throw new TypeError(value?.tag === 'Err' && typeof value.values?.[0] === 'string' ? value.values[0] : 'Invalid Ubi app result');
  }
  return value.values[0];
};
function snapshot(root) {
  const focus = root.ownerDocument.activeElement;
  const nodes = [root, ...root.querySelectorAll('*')].map(element => ({
    element, children: [...element.childNodes], attributes: [...element.attributes].map(a => [a.name, a.value]),
  }));
  const texts = [];
  for (const { children } of nodes) for (const child of children) {
    if (child.nodeType === 3) texts.push([child, child.nodeValue]);
  }
  return () => {
    for (const { element, attributes, children } of nodes) {
      for (const name of element.getAttributeNames()) element.removeAttribute(name);
      for (const [name, value] of attributes) element.setAttribute(name, value);
      element.replaceChildren(...children);
    }
    for (const [text, value] of texts) text.nodeValue = value;
    if (root.contains(focus)) focus.focus({ preventScroll: true });
  };
}
function place(parent, children) {
  for (let i = 0; i < children.length; i++) {
    if (parent.childNodes[i] !== children[i]) parent.insertBefore(children[i], parent.childNodes[i] ?? null);
  }
  while (parent.childNodes.length > children.length) parent.lastChild.remove();
}
export function mount(root, app, options = {}) {
  if (!root || root.nodeType !== 1) throw new TypeError('Ubi UI mount requires an element');
  owners.get(root)?.dispose();
  const id = `ubi-${++nextMount}`, media = root.ownerDocument.defaultView.matchMedia('(prefers-color-scheme: dark)');
  let model, elements = new Map(), bindings = new Map(), revision = 0;
  let processing = false, closed = false, failed = false, themeElement, theme = 'System';
  function report(error) {
    try { options.onError?.(error instanceof Error ? error : new Error(String(error))); }
    catch (fault) { console.error('Ubi UI error handler failed', fault); }
  }
  function applyTheme() {
    if (themeElement) themeElement.setAttribute('data-ubi-theme', (theme === 'System' ? media.matches ? 'Dark' : 'Light' : theme).toLowerCase());
  }
  function patch(view) {
    root.setAttribute('data-ubi-mount', id);
    const next = new Map(), nextBindings = new Map();
    for (let i = 0; i < view.nodes.length; i++) {
      const node = view.nodes[i], old = elements.get(node.key);
      const tag = node.kind === 'Button' ? 'button' : node.kind === 'Heading' ? `h${node.props.Level}` : node.kind === 'Text' ? 'p' : node.kind === 'Badge' ? 'span' : 'div';
      const same = old?.kind === node.kind && old.element.localName === tag;
      const element = same ? old.element : root.ownerDocument.createElement(tag);
      const props = node.props;
      element.setAttribute('data-ubi-key', node.key);
      element.setAttribute('class', `ubi-${node.kind.toLowerCase()}`);
      if (!same) element.id = `${id}-${revision + 1}-${i}`;
      if (props.Content !== undefined && element.textContent !== props.Content) element.textContent = props.Content;
      if (props.Gap) element.setAttribute('data-gap', props.Gap.toLowerCase()); else element.removeAttribute('data-gap');
      if (props.Tone) element.setAttribute('data-tone', props.Tone.toLowerCase()); else element.removeAttribute('data-tone');
      if (node.kind === 'Button') {
        element.type = 'button'; element.disabled = props.Disabled || props.Busy;
        element.setAttribute('data-variant', props.Variant.toLowerCase());
        if (props.Busy) element.setAttribute('aria-busy', 'true'); else element.removeAttribute('aria-busy');
        if (props.Pressed !== undefined) element.setAttribute('aria-pressed', String(props.Pressed)); else element.removeAttribute('aria-pressed');
        nextBindings.set(element, { message: props.Activate, revision: revision + 1 });
      }
      next.set(node.key, { element, kind: node.kind });
    }
    for (const node of view.nodes) {
      if (['Column', 'Row', 'Card'].includes(node.kind)) place(next.get(node.key).element, node.children.map(key => next.get(key).element));
    }
    place(root, [next.get(view.root).element]);
    const nextTheme = next.get(view.root).element;
    for (const { element } of next.values()) if (element !== nextTheme) element.removeAttribute('data-ubi-theme');
    const nextMode = view.nodes[0].props.Theme ?? 'System';
    nextTheme.setAttribute('data-ubi-theme', (nextMode === 'System' ? media.matches ? 'Dark' : 'Light' : nextMode).toLowerCase());
    // Publish bindings only after all potentially throwing DOM operations.
    elements = next; bindings = nextBindings; themeElement = nextTheme; theme = nextMode; revision++;
  }
  function render(candidate) {
    const validated = result(app.validate(encode(candidate)));
    const view = validateView(app.view(encode(validated)));
    const restore = snapshot(root);
    try { patch(view); }
    catch (error) {
      try { restore(); }
      catch (rollbackError) {
        failed = true; disconnect();
        report(new Error('Ubi UI DOM rollback failed; recover the mount', { cause: rollbackError }));
      }
      throw error;
    }
    model = validated;
  }
  function click(event) {
    if (closed || failed || processing || owners.get(root) !== handle) return;
    const element = event.target?.closest?.('button'), binding = bindings.get(element);
    if (!binding || binding.revision !== revision || element.disabled || !root.contains(element)) return;
    processing = true;
    try {
      const normalized = { tag: 'Activate', values: [binding.message] };
      render(result(app.update(encode(model), encode(normalized))));
    } catch (error) { report(error); }
    finally { processing = false; }
  }
  function connect() { root.addEventListener('click', click); media.addEventListener('change', applyTheme); }
  function disconnect() { root.removeEventListener('click', click); media.removeEventListener('change', applyTheme); }
  const handle = {
    dispose() {
      if (closed) return;
      closed = true; disconnect(); bindings.clear();
      if (owners.get(root) === handle) owners.delete(root);
    },
    recover() {
      if (closed || owners.get(root) !== handle || processing) return;
      processing = true;
      try {
        render(model); failed = false; disconnect(); connect();
      } catch (error) { report(error); }
      finally { processing = false; }
    },
  };
  owners.set(root, handle);
  try { render(app.initial()); connect(); }
  catch (error) { handle.dispose(); report(error); throw error; }
  return handle;
}
