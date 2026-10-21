import { arena, node, button, e } from './fixtures.mjs';
const results = document.querySelector('#results'), fixtures = document.querySelector('#fixtures');
let mount;
try { ({ mount } = await import('../web/renderer.mjs')); }
catch (error) { results.textContent = `UBI_UI_BROWSER_FAILED: ${error.message}`; throw error; }
const lines = [];
function assert(value, message = 'Assertion failed') { if (!value) throw Error(message); }
function fixture() { const root = document.createElement('div'); fixtures.append(root); return root; }
function app(options = {}) {
  let updates = 0;
  return {
    get updates() { return updates; },
    initial: () => ({ count: 0 }),
    validate: value => options.badState && JSON.parse(value).count ? e('Err', 'Invalid state') : e('Ok', JSON.parse(value)),
    update(state, event) {
      updates++;
      options.reenter?.();
      const count = JSON.parse(state).count + 1;
      if (options.reject) return e('Err', 'Rejected');
      if (options.throwUpdate) throw Error('Update failed');
      assert(JSON.parse(event).tag === 'Activate');
      return e('Ok', { count });
    },
    view(state) {
      const { count } = JSON.parse(state);
      if (count && options.throwView) throw Error('View failed');
      if (count && options.invalid) return arena([node('root', 'Unknown')]);
      const b = button('add', e(count ? 'Current' : 'Initial'));
      b.props[0] = e('Content', count && options.overflow ? 'x'.repeat(1_048_576) : `Count ${count}`);
      if (options.disabled) b.props[2] = e('Disabled', true);
      if (options.busy) b.props[3] = e('Busy', true);
      const nodes = options.remove && count ? [] : [b];
      return arena([node('root', 'Column', [e('Theme', e(options.theme ?? 'Light'))], nodes.map(x => x.key)), ...nodes]);
    },
  };
}
async function check(name, body) {
  try { await body(); lines.push(`PASS ${name}`); }
  catch (error) { lines.push(`FAIL ${name}: ${error.stack}`); }
  results.textContent = lines.join('\n');
}
await check('component stylesheet leaves the surrounding host page unchanged', () => {
  assert(getComputedStyle(document.body).margin === '13px', 'Host page margin was reset');
  assert(getComputedStyle(document.querySelector('#host-control')).borderRadius === '2px', 'Unrelated host control was restyled');
  const root = fixture(), handle = mount(root, app());
  assert(getComputedStyle(root.querySelector('button')).minHeight === '44px', 'Mounted component did not receive scoped styles');
  handle.dispose();
});
await check('keyed element identity and focus survive a transition', () => {
  const root = fixture(), a = app(), handle = mount(root, a), b = root.querySelector('button');
  b.focus(); b.click();
  assert(a.updates === 1); assert(b === root.querySelector('button'));
  assert(document.activeElement === b); assert(b.textContent === 'Count 1');
  handle.dispose();
});
await check('disabled and busy native buttons do not activate', () => {
  for (const options of [{ disabled: true }, { busy: true }]) {
    const root = fixture(), a = app(options), handle = mount(root, a), b = root.querySelector('button');
    assert(b.disabled); b.click(); assert(a.updates === 0);
    if (options.busy) assert(b.getAttribute('aria-busy') === 'true');
    handle.dispose();
  }
});
await check('two mounts and detached/disposed buttons cannot cross-route', () => {
  const left = fixture(), right = fixture(), a = app({ remove: true }), b = app();
  const l = mount(left, a), r = mount(right, b), old = left.querySelector('button');
  old.click(); old.click(); assert(a.updates === 1); assert(b.updates === 0);
  right.querySelector('button').click(); assert(b.updates === 1);
  const retained = right.querySelector('button'); r.dispose(); retained.click(); assert(b.updates === 1);
  l.dispose();
});
await check('replacing an app disposes previous binding ownership', () => {
  const root = fixture(), a = app(), b = app(); mount(root, a);
  const old = root.querySelector('button'), handle = mount(root, b);
  old.click(); assert(a.updates === 0 && b.updates === 0);
  root.querySelector('button').click(); assert(b.updates === 1); handle.dispose();
});
await check('failed transitions retain committed DOM/state', () => {
  for (const options of [{ reject: true }, { throwUpdate: true }, { throwView: true }, { invalid: true }, { badState: true }, { overflow: true }]) {
    const root = fixture(), a = app(options), errors = [], handle = mount(root, a, { onError: error => errors.push(error) });
    const b = root.querySelector('button'); b.focus(); b.click();
    assert(errors.length === 1); assert(root.querySelector('button') === b);
    assert(b.textContent === 'Count 0'); assert(document.activeElement === b);
    handle.recover(); assert(root.querySelector('button').textContent === 'Count 0'); handle.dispose();
  }
});
await check('DOM patch failure rolls back prior nodes and permits retry', () => {
  const root = fixture(), a = app(), errors = [], handle = mount(root, a, { onError: error => errors.push(error) });
  const b = root.querySelector('button'), original = b.setAttribute;
  let thrown = false;
  b.setAttribute = function (...args) { if (!thrown) { thrown = true; throw Error('Injected patch failure'); } return original.apply(this, args); };
  b.click(); assert(errors.length === 1); assert(b.textContent === 'Count 0');
  b.setAttribute = original; b.click(); assert(b.textContent === 'Count 1'); handle.dispose();
});
await check('rollback failure disposes until explicit recovery', () => {
  const root = fixture(), a = app(), errors = [], handle = mount(root, a, { onError: error => errors.push(error) });
  const b = root.querySelector('button'), original = b.setAttribute;
  b.setAttribute = () => { throw Error('Persistent DOM failure'); };
  b.click(); const before = a.updates; b.click(); assert(a.updates === before);
  assert(errors.length >= 1); b.setAttribute = original;
  handle.recover(); root.querySelector('button').click(); assert(root.querySelector('button').textContent === 'Count 1'); handle.dispose();
});
await check('content is text and theme is scoped per mount', () => {
  const root = fixture(), other = fixture(), a = app({ theme: 'Dark' }), b = app({ theme: 'Light' });
  const original = a.view;
  a.view = state => { const view = original(state); view.nodes[1].props[0] = e('Content', '<img src=x onerror=alert(1)>'); return view; };
  const l = mount(root, a), r = mount(other, b);
  assert(!root.querySelector('img')); assert(root.textContent.includes('<img'));
  assert(root.querySelector('[data-ubi-theme]').dataset.ubiTheme === 'dark');
  assert(other.querySelector('[data-ubi-theme]').dataset.ubiTheme === 'light'); l.dispose(); r.dispose();
});
await check('reorders preserve identity, kind replacement removes old bindings', () => {
  const root = fixture(), a = app();
  a.view = state => {
    const { count } = JSON.parse(state);
    return arena([node('root', 'Row', [], count ? ['other', 'add'] : ['add', 'other']), button('add'),
      count > 1 ? node('other', 'Text', [e('Content', 'Replaced')]) : button('other')]);
  };
  const handle = mount(root, a), add = root.querySelector('[data-ubi-key="add"]'), other = root.querySelector('[data-ubi-key="other"]');
  add.focus(); add.click(); assert(root.querySelector('[data-ubi-key="add"]') === add);
  assert(document.activeElement === add); assert(add.previousSibling === other);
  add.click(); assert(root.querySelector('[data-ubi-key="other"]').localName === 'p');
  other.click(); assert(a.updates === 2); handle.dispose();
});
await check('synchronous reentry is ignored', () => {
  const root = fixture(), options = {}, a = app(options), handle = mount(root, a);
  options.reenter = () => root.querySelector('button').click();
  root.querySelector('button').click(); assert(a.updates === 1); handle.dispose();
});
await check('System theme listener follows media and is removed on disposal', () => {
  const original = window.matchMedia, media = new EventTarget(); let listeners = 0;
  media.matches = false;
  const add = media.addEventListener.bind(media), remove = media.removeEventListener.bind(media);
  media.addEventListener = (...args) => { listeners++; add(...args); };
  media.removeEventListener = (...args) => { listeners--; remove(...args); };
  window.matchMedia = () => media;
  try {
    const root = fixture(), handle = mount(root, app({ theme: 'System' }));
    assert(root.firstChild.dataset.ubiTheme === 'light'); media.matches = true;
    media.dispatchEvent(new Event('change')); assert(root.firstChild.dataset.ubiTheme === 'dark');
    assert(listeners === 1); handle.dispose(); assert(listeners === 0);
  } finally { window.matchMedia = original; }
});
const keyboardRoot = fixture(); keyboardRoot.id = 'keyboard-probe';
mount(keyboardRoot, app());
lines.push('Keyboard probe: use Enter and Space; each should increment once.');
lines.push(`Engine: ${navigator.userAgent}`);
lines.push(lines.some(line => line.startsWith('FAIL')) ? 'UBI_UI_BROWSER_FAILED' : 'UBI_UI_BROWSER_OK');
results.textContent = lines.join('\n');
