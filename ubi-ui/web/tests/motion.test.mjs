import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { MotionState, sample } from '../motion.mjs';

const linear = (durationMs = 100, delayMs = 0) => ({ durationMs, delayMs, easing: 'Linear' });
const target = (key, value, motion = linear(), extra = {}) => ({
  key, kind: 'View', channels: { position: value }, motion, enter: 'None', exit: 'None', ...extra,
});
const exitTarget = (key, value, motion = linear()) => target(key, value, motion, {
  exit: 'Fade', snapshot: { key, kind: 'View', props: {}, children: [] },
});

test('JavaScript sampling matches cross-host fixtures', () => {
  const fixture = JSON.parse(readFileSync(new URL('../../fixtures/motion.json', import.meta.url), 'utf8'));
  assert.ok(Array.isArray(fixture.samples) && fixture.samples.length > 0);
  for (const item of fixture.samples) {
    assert.ok(Math.abs(sample(item.from, item.to, item.startMs, item.nowMs, item.motion) - item.expected) <= 0.00001);
  }
});

test('motion sampling has exact endpoints, delays, zero-duration behavior and shared easing values', () => {
  assert.equal(sample(0, 100, 10, 10, linear()), 0);
  assert.equal(sample(0, 100, 10, 60, linear()), 50);
  assert.equal(sample(0, 100, 10, 110, linear()), 100);
  assert.equal(sample(0, 100, 10, 500, linear()), 100);
  assert.equal(sample(0, 100, 10, 59, linear(100, 50)), 0);
  assert.equal(sample(0, 100, 10, 60, linear(100, 50)), 0);
  assert.equal(sample(0, 100, 10, 60, linear(0, 100)), 100);
  assert.equal(sample(0, 1, 0, 50, { ...linear(), easing: 'EaseOut' }), 0.875);
  assert.equal(sample(0, 1, 0, 50, { ...linear(), easing: 'EaseInOut' }), 0.5);
  const spring = t => 1 - Math.exp(-6 * t) * (Math.cos(10 * t) + 0.6 * Math.sin(10 * t));
  assert.ok(Math.abs(sample(0, 1, 0, 50, { ...linear(), easing: 'Spring' }) - spring(0.5) / spring(1)) < 1e-12);
  assert.equal(sample(0, 1, 0, 100, { ...linear(), easing: 'Spring' }), 1);
});

test('retargeting samples the visible value, including a delayed interruption', () => {
  const state = new MotionState();
  state.commit([target('item', 0, linear(0))], 0, false);
  state.commit([target('item', 100)], 0, false);
  assert.equal(state.sample(25, false).channels.get('item').position, 25);
  state.commit([target('item', 0, linear(100, 50))], 25, false);
  assert.equal(state.sample(74, false).channels.get('item').position, 25);
  assert.equal(state.sample(75, false).channels.get('item').position, 25);
  assert.equal(state.sample(125, false).channels.get('item').position, 12.5);
  assert.equal(state.sample(175, false).channels.get('item').position, 0);
  state.dispose();
  assert.equal(state.sample(200, false).active, false);
});

test('ticker retargeting accepts fractional presentation values but keeps integer targets', () => {
  const ticker = value => ({ ...target('ticker', 0), channels: { number: value } });
  const state = new MotionState();
  state.commit([ticker(128)], 0);
  state.commit([ticker(129)], 0);
  assert.equal(state.sample(25).channels.get('ticker').number, 128.25);
  state.commit([ticker(128)], 25);
  assert.equal(state.sample(75).channels.get('ticker').number, 128.125);
  assert.equal(state.sample(125).channels.get('ticker').number, 128);
  assert.throws(() => state.commit([ticker(128.5)], 125), TypeError);
});

test('reduced motion settles tracks and disposes their deadlines immediately', () => {
  const state = new MotionState();
  state.commit([target('item', 0, linear(0))], 0, false);
  state.commit([target('item', 100)], 0, false);
  assert.equal(state.sample(10, false).active, true);
  const reduced = state.sample(10, true);
  assert.equal(reduced.channels.get('item').position, 100);
  assert.equal(reduced.active, false);
  assert.equal(reduced.nextWakeMs, null);
  assert.equal(state.sample(10, false).channels.get('item').position, 100);
});

test('invalid targets fail before changing existing tracks', () => {
  const state = new MotionState();
  state.commit([target('item', 0, linear(0))], 0, false);
  state.commit([target('item', 100)], 0, false);
  assert.throws(() => state.commit([target('item', Number.NaN)], 25, false), TypeError);
  assert.equal(state.sample(25, false).channels.get('item').position, 25);
  const many = Object.fromEntries(Array.from({ length: 13 }, (_, i) => [`channel-${i}`, i]));
  assert.throws(() => state.commit([{ ...target('item', 0), channels: many }], 25, false), TypeError);
  const rawLimit=Object.fromEntries(Array.from({length:16},(_,i)=>[`custom-${i}`,i]));
  assert.throws(() => state.commit([{...target('item',0),channels:rawLimit}],25,false),TypeError);
  for (const [name, value] of [['opacity',2],['scale',0.2],['switch-position',-1],['progress',101],['disclosure-height',100001]]) {
    assert.throws(() => state.commit([{ ...target('item', 0), channels: { [name]:value } }], 25, false), TypeError, name);
  }
  assert.equal(state.sample(25, false).channels.get('item').position, 25);
});

test('exit snapshots omit editable subtrees and remain visual-only until their deadline', () => {
  const state = new MotionState();
  const snapshot = {
    key: 'card', kind: 'Card', props: { Content: 'kept', Activate: { secret: 'message' } },
    children: [{ key: 'secret-input', kind: 'PasswordInput', props: { Value: 'secret' }, children: [] }],
  };
  state.commit([target('card', 1, linear(100), { exit: 'Fade', snapshot })], 0, false);
  state.commit([], 0, false);
  const exiting = state.sample(50, false);
  assert.equal(exiting.exits.length, 1);
  assert.equal(exiting.exits[0].snapshot.children.length, 0);
  assert.equal('Activate' in exiting.exits[0].snapshot.props, false);
  assert.equal(exiting.exits[0].channels.opacity, 0.5);
  assert.equal(exiting.active, true);
  const settled = state.sample(100, false);
  assert.equal(settled.exits.length, 0);
  assert.equal(settled.active, false);
});

test('motion state bounds visual exits and same-key resurrection reuses presentation', () => {
  const state = new MotionState();
  state.commit([exitTarget('same', 0, linear(0))], 0, false);
  state.commit([exitTarget('same', 100)], 0, false);
  assert.equal(state.sample(25, false).channels.get('same').position, 25);
  state.commit([], 25, false);
  assert.equal(state.sample(40, false).exits[0].channels.opacity, 0.85);
  state.commit([exitTarget('same', 100)], 50, false);
  const resurrected = state.sample(50, false).channels.get('same');
  assert.equal(resurrected.position, 25);
  assert.equal(resurrected.opacity, 0.75);
  state.commit(Array.from({ length: 4096 }, (_, i) => exitTarget(`k${i}`, 0)), 40, false);
  state.commit([], 40, false);
  assert.equal(state.sample(40, false).exits.length, 4096);
});

test('visual snapshots cap recursively and aggregate exits evict oldest snapshots', () => {
  const state = new MotionState();
  const snapshot = (key, children) => ({
    key, kind:'View', props:{}, children:Array.from({length:children},(_,index)=>({key:`${key}-${index}`,kind:'Text',props:{},children:[]})),
  });
  assert.throws(() => state.commit([target('oversized',0,linear(),{exit:'Fade',snapshot:snapshot('oversized',4096)})],0,false),TypeError);
  const targets=Array.from({length:5},(_,index)=>target(`root-${index}`,0,linear(),{exit:'Fade',snapshot:snapshot(`root-${index}`,999)}));
  state.commit(targets,0,false); state.commit([],0,false);
  const exits=state.sample(10,false).exits;
  assert.equal(exits.length,4);
  assert.ok(!exits.some(exit=>exit.key==='root-0'));
  const count=tree=>1+tree.children.reduce((total,child)=>total+count(child),0);
  assert.ok(exits.reduce((total,exit)=>total+count(exit.snapshot),0)<=4096);
});

test('valid channel boundaries remain valid when transitions synthesize offsets', () => {
  const state=new MotionState();
  state.commit([{...target('rise',0,linear(0)),channels:{'translate-y':100000},enter:'Rise'}],0,false);
  assert.equal(state.sample(0,false).channels.get('rise')['translate-y'],100000);
  state.commit([{...target('scale',0,linear(0)),channels:{scale:0.25},enter:'Scale'}],1,false);
  assert.equal(state.sample(1,false).channels.get('scale').scale,0.25);
  state.commit([{...target('exit',0,linear(100)),channels:{'translate-y':100000},exit:'Rise',snapshot:{key:'exit',kind:'View',props:{},children:[]}}],2,false);
  state.commit([],3,false);
  assert.equal(state.sample(4,false).exits[0].channels['translate-y'],100000);
});
