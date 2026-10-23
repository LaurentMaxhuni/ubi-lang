import { mount } from '../web/renderer.mjs';

const output = document.getElementById('results');
const fixtures = document.getElementById('fixtures');
const manifest = await (await fetch('../docs/public/previews/manifest.json')).json();
const results = [];
const assert = (condition, message) => { if (!condition) throw new Error(message); };
const byKey = (root, key) => root.querySelector(`[data-ubi-key="${key}"]`);
async function test(slug, name, check) {
  const root = document.createElement('div');
  root.style.width = '640px';
  fixtures.append(root);
  let handle;
  const faults = [];
  try {
    const example = manifest.find(item => item.slug === slug);
    assert(example, `missing compiled ${slug}`);
    const app = await import(new URL(`../docs/public/previews/${example.module}`, import.meta.url));
    handle = mount(root, app, { motion: 'reduced', onError: error => faults.push(error.message) });
    await check(root, handle);
    assert(!faults.length, faults.join('; '));
    results.push({ name, ok: true });
  } catch (error) {
    results.push({ name, ok: false, error: `${error.stack}${faults.length ? `\nRuntime: ${faults.join('; ')}` : ''}` });
  } finally {
    handle?.dispose();
    root.remove();
  }
  output.textContent = results.map(item => `${item.ok ? 'PASS' : 'FAIL'} ${item.name}${item.error ? `\n${item.error}` : ''}`).join('\n');
}

await test('dialog', 'modal open, Escape and focus return use the real reducer', (root, handle) => {
  const trigger = byKey(root, 'open');
  trigger.focus();
  trigger.click();
  assert(handle.getModel() === true, 'dialog did not open');
  const close = byKey(root, 'close');
  assert(document.activeElement === close, 'focus did not enter modal');
  close.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
  assert(handle.getModel() === false, 'Escape did not close dialog');
  assert(document.activeElement === trigger, 'focus did not return to trigger');
});

await test('alert-dialog', 'alert dialog requires an explicit action', (root, handle) => {
  byKey(root, 'open').click();
  const before = JSON.stringify(handle.getModel());
  document.activeElement.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
  assert(JSON.stringify(handle.getModel()) === before, 'Escape dismissed the alert dialog');
});

await test('select', 'selection ignores disabled options and commits a typed value', (root, handle) => {
  byKey(root, 'frequency-trigger').click();
  assert(handle.getModel().open, 'select did not open');
  assert(document.activeElement === byKey(root, 'frequency-option-weekly'), 'select did not focus its current option');
  byKey(root, 'frequency-option-never').click();
  assert(handle.getModel().selected === 'weekly', 'disabled choice changed value');
  const weekly = byKey(root, 'frequency-option-weekly');
  weekly.focus();
  weekly.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true, cancelable: true }));
  const daily = byKey(root, 'frequency-option-daily');
  assert(document.activeElement === daily && handle.getModel().open, 'navigation should skip disabled choices without closing');
  daily.click();
  assert(handle.getModel().selected === 'daily' && !handle.getModel().open, 'typed selection did not commit');
  assert(document.activeElement === byKey(root, 'frequency-trigger'), 'selection did not return focus to its trigger');
});

await test('combobox', 'query filters actual Ubi options and selection retains input focus', (root, handle) => {
  const input = root.querySelector('input');
  input.focus();
  input.value = 'paper';
  input.dispatchEvent(new InputEvent('input', { bubbles: true }));
  assert(handle.getModel().query === 'paper', 'query did not reach the Ubi reducer');
  const option = byKey(root, 'theme-option-paper');
  assert(option, 'matching option missing');
  assert(!byKey(root, 'theme-option-night'), 'nonmatching option retained');
  assert(input.getAttribute('role') === 'combobox' && input.getAttribute('aria-expanded') === 'true', 'combobox input semantics missing');
  assert(input.getAttribute('aria-controls') === byKey(root, 'theme-list').id, 'combobox input does not control its listbox');
  input.setSelectionRange(3, 3);
  input.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true, cancelable: true }));
  assert(document.activeElement === input && input.selectionStart === 3, 'list navigation moved the query focus/caret');
  assert(input.getAttribute('aria-activedescendant') === option.id, 'matching option is not the active descendant');
  input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', isComposing: true, bubbles: true, cancelable: true }));
  assert(handle.getModel().open, 'IME composition committed a choice');
  input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true }));
  assert(handle.getModel().selected === 'paper', 'option value did not commit');
  assert(document.activeElement === input, 'combobox lost input focus');
  assert(input.getAttribute('aria-expanded') === 'false' && !input.hasAttribute('aria-activedescendant'), 'closed combobox retained active option semantics');
});

await test('toast', 'notification has an explicit working dismissal', (root, handle) => {
  byKey(root, 'show').click();
  assert(handle.getModel() === true, 'toast did not open');
  byKey(root, 'close').click();
  assert(handle.getModel() === false, 'toast did not close');
});

await test('tooltip', 'focus requests a closed tooltip and blur dismisses it', (root, handle) => {
  const anchor = byKey(root, 'help');
  const outside = document.createElement('button');
  outside.textContent = 'Outside fixture';
  document.body.append(outside);
  try {
    anchor.focus();
    assert(handle.getModel() === true, 'focus OpenRequest did not reach the source reducer');
    assert(anchor.getAttribute('aria-describedby')?.includes(byKey(root, 'help-tip').id), 'tooltip description missing');
    outside.focus();
    assert(handle.getModel() === false, 'blur did not dismiss the tooltip');
  } finally { outside.remove(); }
});

await test('context-menu', 'Shift+F10 opens a closed context menu and actions return focus', (root, handle) => {
  const anchor = byKey(root, 'project');
  anchor.focus();
  anchor.dispatchEvent(new KeyboardEvent('keydown', { key: 'F10', shiftKey: true, bubbles: true, cancelable: true }));
  assert(handle.getModel().open, 'context OpenRequest did not reach the source reducer');
  const action = byKey(root, 'project-menu-item-copy');
  assert(document.activeElement === action, 'context menu did not focus its first action');
  action.click();
  assert(!handle.getModel().open && document.activeElement === anchor, 'context action did not restore trigger focus');
});

await test('carousel', 'carousel advances explicitly and respects boundaries', (root, handle) => {
  assert(handle.getModel() === 0, 'unexpected initial slide');
  byKey(root, 'highlights-previous').click();
  assert(handle.getModel() === 0, 'previous crossed first boundary');
  byKey(root, 'highlights-next').click();
  assert(handle.getModel() === 1, 'next did not select slide');
  byKey(root, 'highlights-next').click();
  byKey(root, 'highlights-next').click();
  assert(handle.getModel() === 2, 'next crossed last boundary');
});

await test('number-ticker', 'ticker reducer keeps the committed numeric value', (root, handle) => {
  const before = handle.getModel();
  byKey(root, 'increment').click();
  assert(handle.getModel() === before + 1, 'ticker did not increment');
  handle.setMotion('full');
  byKey(root, 'decrement').click();
  assert(handle.getModel() === before, 'presentation changed the logical number');
  handle.setMotion('reduced');
});

document.body.dataset.testStatus = results.every(item => item.ok) ? 'passed' : 'failed';
window.__ubiInteractionTests = results;
