import { mount } from './renderer.mjs';

const root = document.getElementById('app');
const error = document.getElementById('error');
const slug = root.dataset.slug;
const params = new URLSearchParams(location.search);
const validTheme = value => ['light', 'dark', 'system'].includes(value);
let theme = validTheme(params.get('theme')) ? params.get('theme') : 'system';
let handle;
function report(fault) {
  const message = fault instanceof Error ? fault.message : String(fault);
  error.textContent = `Unable to render this Ubi example: ${message}`;
  error.hidden = false;
  document.documentElement.dataset.ubiStatus = 'error';
  if (parent !== window) parent.postMessage({ type: 'ubi-preview-status', status: 'error', message }, location.origin);
}
try {
  if (!/^[a-z0-9-]+$/.test(slug)) throw new TypeError('Invalid example slug');
  const app = await import(new URL(`../${slug}/app.mjs`, import.meta.url));
  handle = mount(root, app, { theme, preview: params.get('preview') === 'true', onError: report });
  document.documentElement.dataset.ubiStatus = 'ready';
  if (parent !== window) parent.postMessage({ type: 'ubi-preview-status', status: 'ready' }, location.origin);
  // The documentation owns this frame. Other windows cannot change its state.
  addEventListener('message', event => {
    if (event.source !== parent || event.origin !== location.origin || !event.data || event.data.type !== 'ubi-preview-theme' || !validTheme(event.data.theme)) return;
    theme = event.data.theme;
    handle.setTheme(theme);
  });
  addEventListener('pagehide', () => handle.dispose(), { once: true });
} catch (fault) { report(fault); }
