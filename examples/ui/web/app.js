import { mount } from './renderer.mjs';
const boundary = document.querySelector('#error-boundary');
const message = document.querySelector('#error-message');
let handle;
function onError(error) {
  boundary.hidden = false;
  message.textContent = `The interface could not update: ${error.message}`;
}
try {
  const app = await import('/__ubi/modules/src/main.mjs');
  handle = mount(document.querySelector('#app'), app, { onError });
  document.querySelector('#recover').addEventListener('click', () => {
    boundary.hidden = true;
    handle.recover();
  });
} catch (error) { onError(error); document.querySelector('#recover').disabled = true; }
window.addEventListener('pagehide', () => handle?.dispose());
// A bfcache-restored document needs fresh binding ownership.
window.addEventListener('pageshow', event => { if (event.persisted) location.reload(); });
