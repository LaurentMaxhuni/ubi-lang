const form = document.querySelector('#add-task');
const title = document.querySelector('#title');
const list = document.querySelector('#tasks');
const status = document.querySelector('#status');
const error = document.querySelector('#error');
const empty = document.querySelector('#empty');
let nextId = 1;
const tasks = [];

function render() {
  list.replaceChildren();
  empty.hidden = tasks.length !== 0;
  const completed = tasks.filter(task => task.completed).length;
  status.textContent = tasks.length ? `${completed} of ${tasks.length} completed` : 'Ready when you are.';
  for (const [index, task] of tasks.entries()) {
    const row = document.createElement('li');
    row.classList.toggle('completed', task.completed);
    const text = document.createElement('span');
    text.className = 'task-title';
    text.textContent = task.title;
    const button = document.createElement('button');
    button.type = 'button';
    button.textContent = task.completed ? 'Completed' : 'Complete';
    button.disabled = task.completed;
    button.setAttribute('aria-label', `${button.textContent}: ${task.title}`);
    button.addEventListener('click', () => {
      try {
        tasks[index] = completeTask(JSON.stringify(task));
        render();
        title.focus();
      } catch (cause) { showError(cause); }
    });
    row.append(text, button);
    list.append(row);
  }
}

function showError(cause) {
  error.hidden = false;
  error.textContent = `Task could not be updated: ${cause?.message ?? String(cause)}`;
}

let createTask, completeTask;
try {
  ({ createTask, completeTask } = await import('/__ubi/modules/src/main.mjs'));
  form.addEventListener('submit', event => {
    event.preventDefault();
    const value = title.value.trim();
    if (!value) { title.setCustomValidity('Enter a task title.'); title.reportValidity(); return; }
    try {
      tasks.push(createTask(nextId++, value));
      error.hidden = true;
      title.value = '';
      render();
      title.focus();
    } catch (cause) { showError(cause); }
  });
  title.addEventListener('input', () => title.setCustomValidity(''));
} catch (cause) {
  form.querySelector('button').disabled = true;
  showError(cause);
}
