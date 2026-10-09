import { storage } from './storage.js';
const domain = await import('/__ubi/modules/src/main.mjs').catch(cause => {
  const message = document.querySelector('#error');
  message.hidden = false;
  message.textContent = `Cannot load task rules: ${cause.message ?? cause}`;
  throw cause;
});
const form = document.querySelector('#add-task');
const title = document.querySelector('#title');
const list = document.querySelector('#tasks');
const error = document.querySelector('#error');
const selection = document.querySelector('#filter');
let tasks = [];
let busy = false;
let readable = false;

function unwrap(result) {
  if (result.tag !== 'Ok') throw new Error(`Task validation failed: ${result.values[0].tag}`);
  return result.values[0];
}
function showError(cause) {
  error.hidden = false;
  error.textContent = `${cause?.message ?? cause}. Saved tasks have not been replaced.`;
}
function render() {
  list.replaceChildren();
  const shown = domain.filterTasks(JSON.stringify(tasks), selection.value);
  document.querySelector('#empty').hidden = shown.length !== 0;
  const complete = domain.filterTasks(JSON.stringify(tasks), 'completed').length;
  document.querySelector('#status').textContent = `${complete} of ${tasks.length} completed`;
  for (const task of shown) {
    const row = document.createElement('li');
    row.classList.toggle('completed', task.completed);
    const text = document.createElement('span');
    text.className = 'task-title';
    text.textContent = task.title;
    const complete = document.createElement('button');
    complete.type = 'button';
    complete.textContent = task.completed ? 'Completed' : 'Complete';
    complete.disabled = task.completed || busy || !readable;
    complete.setAttribute('aria-label', `Complete: ${task.title}`);
    complete.addEventListener('click', () => change(() => domain.complete(JSON.stringify(tasks), task.id)));
    const remove = document.createElement('button');
    remove.type = 'button';
    remove.className = 'delete';
    remove.textContent = 'Delete';
    remove.disabled = busy || !readable;
    remove.setAttribute('aria-label', `Delete: ${task.title}`);
    remove.addEventListener('click', () => change(() => domain.deleteTask(JSON.stringify(tasks), task.id)));
    row.append(text, complete, remove);
    list.append(row);
  }
  form.querySelector('button').disabled = busy || !readable;
  document.querySelector('#reload').disabled = busy;
}
async function load() {
  if (busy) return;
  busy = true;
  render();
  try {
    const saved = await storage.get();
    const decoded = unwrap(domain.decodeTasks(saved ?? '[]'));
    tasks = decoded;
    readable = true;
    error.hidden = true;
  } catch (cause) { readable = false; showError(cause); }
  finally { busy = false; render(); }
}
async function change(operation) {
  if (busy || !readable) return;
  busy = true;
  render();
  try {
    const next = unwrap(operation());
    await storage.set(JSON.stringify(next));
    tasks = next;
    error.hidden = true;
    title.value = '';
    title.focus();
  } catch (cause) { showError(cause); }
  finally { busy = false; render(); }
}
form.addEventListener('submit', event => {
  event.preventDefault();
  void change(() => domain.create(JSON.stringify(tasks), domain.nextId(JSON.stringify(tasks)), title.value));
});
selection.addEventListener('change', render);
document.querySelector('#reload').addEventListener('click', () => void load());
document.addEventListener('visibilitychange', () => { if (!document.hidden) void load(); });
window.addEventListener('pageshow', () => void load());
await load();
