import test from 'node:test';
import assert from 'node:assert/strict';
import { encode } from '../web/codec.mjs';

test('ABI preserves unit, special floats, negative zero and Unicode', () => {
  assert.equal(encode([undefined, NaN, Infinity, -Infinity, -0, 'Café 😀']),
    '[null,"NaN","+Infinity","-Infinity","-0","Café 😀"]');
  assert.equal(encode({ tag: 'Activate', values: [{ tag: 'Save', values: [] }] }),
    '{"tag":"Activate","values":[{"tag":"Save","values":[]}]}');
});
test('UTF-8 budget includes escaping and string delimiters exactly', () => {
  assert.equal(encode('a'.repeat(1048574)).length, 1048576);
  assert.throws(() => encode('a'.repeat(1048575)), /budget/i);
  assert.equal(new TextEncoder().encode(encode('😀'.repeat(262143) + 'aa')).length, 1048576);
  assert.throws(() => encode('😀'.repeat(262144)), /budget/i);
  assert.equal(encode('\n'.repeat(524287)).length, 1048576);
  assert.throws(() => encode('\n'.repeat(524288)), /budget/i);
});
test('own data only, no accessors, unsupported values or invalid Unicode', () => {
  let reads = 0;
  const getter = Object.defineProperty({}, 'x', { enumerable: true, get() { reads++; return 1; } });
  for (const value of [getter, Object.create({ x: 1 }), new Date(), () => {}, Symbol(), 1n, '\ud800']) {
    assert.throws(() => encode(value), TypeError);
  }
  assert.equal(reads, 0);
  const inherited = Object.create({ get bad() { reads++; return 1; } });
  assert.throws(() => encode(inherited), TypeError);
  assert.equal(reads, 0);
});
test('cycles rejected, shared acyclic data allowed, expansion bounded', () => {
  const cyclic = []; cyclic.push(cyclic);
  assert.throws(() => encode(cyclic), /cyclic/i);
  const shared = { x: 4 };
  assert.equal(encode([shared, shared]), '[{"x":4},{"x":4}]');
  let dag = ['x'.repeat(1024)];
  for (let i = 0; i < 20; i++) dag = [dag, dag];
  assert.throws(() => encode(dag), /budget/i);
});
test('32 aggregate levels accepted and 33 rejected', () => {
  let value = 1;
  for (let i = 0; i < 32; i++) value = [value];
  assert.equal(JSON.parse(encode(value)).length, 1);
  assert.throws(() => encode([value]), /depth/i);
});
