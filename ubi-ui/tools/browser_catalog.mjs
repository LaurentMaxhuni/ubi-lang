import { mount, validateView } from '../docs/public/previews/runtime/renderer.mjs';
const output = document.getElementById('results');
const fixtures = document.getElementById('fixtures');
const manifest = await (await fetch('../docs/public/previews/manifest.json')).json();
const results = [];
const assert = (condition, message) => { if (!condition) throw Error(message); };
function assertRenderedContent(root, handle) {
  for (const node of validateView(handle.getView()).nodes) {
    if (node.children.length || !node.props.Content) continue;
    const element = [...root.querySelectorAll('[data-ubi-key]')].find(element => element.dataset.ubiKey === node.key);
    const input = element?.querySelector('input,textarea');
    if (input && ['TextInput','PasswordInput','NumberInput','TextArea'].includes(node.kind)) {
      assert(input.getAttribute('placeholder') === node.props.Content, `${node.key}: source placeholder missing`);
    } else assert(element?.textContent.includes(node.props.Content), `${node.key}: source text missing from rendered content`);
  }
}
for (const example of manifest) {
  const root = document.createElement('div');
  fixtures.append(root);
  let handle;
  try {
    const app = await import(new URL(`../docs/public/previews/${example.module}`, import.meta.url));
    for (const motion of ['full', 'reduced']) for (const width of [320, 768, 1440]) {
      root.style.width = `${width}px`;
      const faults = [];
      handle = mount(root, app, { preview: true, theme: 'light', motion, onError: error => faults.push(error.message) });
      const appRoot = root.querySelector(`[data-ubi-key="${handle.getView().root}"]`);
      assert(appRoot, 'missing real component tree');
      assertRenderedContent(root, handle);
      assert(root.scrollWidth <= width + 1, `${width}px overflow (${root.scrollWidth}px)`);
      const control = (example.slug === 'dock' ? root.querySelector('[data-ubi-key="workspace-dock-activity"]') : null) ?? [...root.querySelectorAll('input:not(:disabled), textarea:not(:disabled), button:not(:disabled)')].find(control => control.getClientRects().length && !control.closest('[inert]') && !control.readOnly && !(control.type === 'radio' && control.checked) && control.getAttribute('aria-selected') !== 'true' && control.getAttribute('aria-pressed') !== 'true');
      if (control && width === 320) {
        const before = JSON.stringify(handle.getModel());
        if (control.matches('input[type=checkbox], input[type=radio], button')) control.click();
        else {
          control.value = control.type === 'range' ? '73' : control.type === 'number' ? '9' : 'Browser 🦀';
          control.dispatchEvent(new InputEvent('input', { bubbles: true }));
        }
        assert(!faults.length, faults.join(', '));
        assert(before !== JSON.stringify(handle.getModel()), 'real reducer did not change model');
        assertRenderedContent(root, handle);
      }
      handle.setTheme('dark');
      assert(appRoot.dataset.ubiTheme === 'dark', 'theme did not apply');
      handle.setMotion(motion === 'full' ? 'reduced' : 'full');
      assert(!faults.length, faults.join(', '));
      handle.dispose(); handle = null;
    }
    results.push(`PASS ${example.slug}: real Ubi, 320/768/1440px, light/dark, full/reduced motion${root.querySelector('button,input,textarea') ? ', interaction' : ''}`);
  } catch (error) { results.push(`FAIL ${example.slug}: ${error.message}`); }
  finally { handle?.dispose(); root.remove(); }
  output.textContent = results.join('\n');
}
document.body.dataset.testStatus = results.some(result => result.startsWith('FAIL')) ? 'failed' : 'passed';
output.textContent += `\n${results.filter(result => result.startsWith('PASS')).length}/${manifest.length} compiled examples passed`;
