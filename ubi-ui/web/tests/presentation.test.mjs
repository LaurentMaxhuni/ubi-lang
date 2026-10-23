import test from 'node:test';
import assert from 'node:assert/strict';
import { createPresentation } from '../presentation.mjs';

const linear = durationMs => ({ durationMs, delayMs: 0, easing: 'Linear' });

function fixture() {
  const attributes = new Map();
  const element = localName => {
    const styleValues = new Map();
    return {
      nodeType: 1, localName, style: { setProperty(name, value) { styleValues.set(name, String(value)); }, removeProperty(name) { styleValues.delete(name); } }, styleValues, hidden: false, inert: false,
      setAttribute(name, value) { attributes.set(name, String(value)); },
      getAttribute(name) { return attributes.get(name) ?? null; },
      removeAttribute(name) { attributes.delete(name); },
      getBoundingClientRect() { return { left: 0, top: 0, width: 1, height: 1 }; },
    };
  };
  let now = 0, nextFrame = 0;
  const pending = new Map();
  const doc = {
    visibilityState: 'visible', addEventListener() {}, removeEventListener() {},
    defaultView: { matchMedia: () => ({ matches: false, addEventListener() {}, removeEventListener() {} }) },
  };
  const root = { nodeType: 1, ownerDocument: doc, addEventListener() {}, removeEventListener() {}, append() {} };
  const presentation = createPresentation(root, {
    motion: 'full',
    clock: {
      now: () => now,
      requestFrame(callback) { const id = ++nextFrame; pending.set(id, callback); return id; },
      cancelFrame(id) { pending.delete(id); },
    },
  });
  return {
    presentation, element, attributes, pending,
    setNow(value) { now = value; },
  };
}

test('ticker and progress frames interpolate visuals while committed semantics stay stable', () => {
  const f = fixture();
  const tickerElement = f.element('div'), tickerVisual = { textContent: '0' }, tickerSemantic = { textContent: '0' };
  const progressElement = f.element('div'), progressFill = f.element('span');
  const entries = new Map([
    ['ticker', { element: tickerElement, numberVisual: tickerVisual, semantic: tickerSemantic }],
    ['progress', { element: progressElement, progressFill }],
  ]);
  const view = (number, progress, duration) => ({ nodes: [
    { key: 'ticker', kind: 'EffectSurface', children: [], props: { Effect: 'NumberTicker', Number: number, Motion: linear(duration) } },
    { key: 'progress', kind: 'ProgressBar', children: [], props: { Progress: progress, Motion: linear(duration) } },
  ] });

  f.presentation.commit(f.presentation.prepare(view(0, 0, 0)), entries);
  const semanticValue = '100';
  tickerSemantic.textContent = semanticValue;
  progressElement.setAttribute('aria-valuenow', semanticValue);
  f.presentation.commit(f.presentation.prepare(view(100, 100, 100)), entries);
  f.presentation.sample(50);

  assert.equal(tickerVisual.textContent, '50');
  assert.equal(tickerSemantic.textContent, '100');
  assert.equal(tickerElement.styleValues.has('--ubi-progress-visual'), false);
  assert.equal(progressElement.getAttribute('aria-valuenow'), '100');
  assert.equal(progressFill.styleValues.get('--ubi-progress-visual'), '50%');
  f.presentation.dispose();
});

test('NumberTicker handles negative half values with native-compatible rounding', () => {
  const f = fixture(), visual = { textContent: '0' };
  const entries = new Map([['ticker', { element: f.element('div'), numberVisual: visual }]]);
  const view = (number, duration) => ({ nodes: [{ key: 'ticker', kind: 'EffectSurface', children: [], props: { Effect: 'NumberTicker', Number: number, Motion: linear(duration) } }] });
  f.presentation.commit(f.presentation.prepare(view(0, 0)), entries);
  f.presentation.commit(f.presentation.prepare(view(-1, 100)), entries);
  f.presentation.sample(50);
  assert.equal(visual.textContent, '-1');
  f.presentation.dispose();
});

test('source EffectSurface NumberTicker receives its default numeric animation channel', () => {
  const f=fixture(),visual={textContent:'0'},semantic={textContent:'0'},entries=new Map([['ticker',{element:f.element('div'),numberVisual:visual,semantic}]]);
  const view=number=>({nodes:[{key:'ticker',kind:'EffectSurface',children:[],props:{Effect:'NumberTicker',Number:number}}]});
  const initial=f.presentation.prepare(view(0));
  assert.equal(initial.targets[0].channels.number,0);
  assert.equal(initial.targets[0].motion.durationMs,220);
  f.presentation.commit(initial,entries);
  semantic.textContent='100';
  f.presentation.commit(f.presentation.prepare(view(100)),entries);
  f.presentation.sample(110);
  assert.equal(visual.textContent,'88');
  assert.equal(semantic.textContent,'100');
  f.presentation.dispose();
});

test('AnimatedList applies list entrances only to direct keyed children', () => {
  const f = fixture();
  const view = { nodes: [
    { key: 'list', kind: 'EffectSurface', props: { Effect: 'AnimatedList' }, children: ['row'] },
    { key: 'row', kind: 'Row', props: {}, children: ['label'] },
    { key: 'label', kind: 'Text', props: {}, children: [] },
  ] };
  const prepared = f.presentation.prepare(view, new Map());
  const targets = new Map(prepared.targets.map(target => [target.key, target]));
  assert.equal(targets.get('row').enter, 'Rise');
  assert.equal(targets.get('row').exit, 'Fade');
  assert.equal(targets.get('label').enter, 'None');
  assert.equal(targets.get('label').exit, 'None');
  f.presentation.dispose();
});

test('TextReveal does not add a whole-surface rise over its word animation', () => {
  const f=fixture();
  const view={nodes:[{key:'reveal',kind:'EffectSurface',children:[],props:{Effect:'TextReveal',Content:'A full sentence',Enter:'Rise'}}]};
  const prepared=f.presentation.prepare(view,new Map());
  assert.equal(prepared.targets.find(target=>target.key==='reveal').enter,'None');
  f.presentation.dispose();
});
