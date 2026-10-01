const root = document.documentElement;
const motionButton = document.querySelector('.motion-toggle');
const reducedMotion = matchMedia('(prefers-reduced-motion: reduce)');
let paused = reducedMotion.matches;

function setMotion(value) {
  paused = value;
  root.dataset.paused = String(paused);
  motionButton.setAttribute('aria-pressed', String(paused));
  motionButton.setAttribute('aria-label', paused ? 'Resume animations' : 'Pause animations');
  motionButton.querySelector('.pause-icon').textContent = paused ? '▷' : 'Ⅱ';
  motionButton.querySelector('.motion-label').textContent = paused ? 'Resume motion' : 'Pause motion';
}
setMotion(paused);
motionButton.addEventListener('click', () => setMotion(!paused));
reducedMotion.addEventListener('change', event => setMotion(event.matches));

const copyButton = document.querySelector('#copy-command');
copyButton.addEventListener('click', async () => {
  try {
    await navigator.clipboard.writeText('cargo install --path .');
    copyButton.textContent = 'Copied';
    document.querySelector('#copy-status').textContent = 'Install command copied to clipboard.';
  } catch {
    document.querySelector('#copy-status').textContent = 'Copy unavailable. Select the command above and copy it manually.';
  }
});

const energy = document.querySelector('#energy');
const runtimeStatus = document.querySelector('#runtime-status');
const error = document.querySelector('#demo-error');
try {
  const { signal, targetNote } = await import('/__ubi/modules/src/main.mjs');
  function renderSignal() {
    try {
      const result = signal(Number(energy.value));
      root.style.setProperty('--radius', `${result.radius}px`);
      root.style.setProperty('--duration', `${result.duration}s`);
      document.querySelector('#energy-value').value = `${energy.value}%`;
      document.querySelector('#mood').textContent = result.label;
      document.querySelector('#radius').textContent = `${result.radius}px`;
      document.querySelector('#duration').textContent = `${result.duration}s`;
      error.hidden = true;
    } catch (cause) {
      error.textContent = `Could not update the orbit: ${cause.message ?? String(cause)}`;
      error.hidden = false;
    }
  }
  renderSignal();
  energy.disabled = false;
  runtimeStatus.textContent = 'Ubi connected';
  energy.addEventListener('input', renderSignal);
  document.querySelectorAll('[data-target]').forEach(button => {
    button.addEventListener('click', () => {
      document.querySelectorAll('[data-target]').forEach(option => {
        option.setAttribute('aria-pressed', String(option === button));
      });
      document.querySelector('#target-note').textContent = targetNote(button.dataset.target);
    });
  });
} catch (cause) {
  runtimeStatus.textContent = 'Ubi unavailable';
  error.textContent = `Start this page with the Ubi dev server to connect the demo. ${cause.message ?? String(cause)}`;
  error.hidden = false;
  document.querySelectorAll('[data-target]').forEach(button => { button.disabled = true; });
}
