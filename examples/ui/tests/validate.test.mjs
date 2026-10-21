import test from 'node:test';
import assert from 'node:assert/strict';
import { validateView } from '../web/validate.mjs';
import { e, node, arena, button } from './fixtures.mjs';

test('normalizes a known arena, freezes result, preserves message data', () => {
  const result = validateView(arena([node('root', 'Column', [e('Gap', e('Md')), e('Theme', e('Dark'))], ['save']), button('save')]));
  assert.equal(result.root, 'root');
  assert.equal(result.nodes[1].kind, 'Button');
  assert.equal(result.nodes[1].props.Activate.tag, 'Increment');
  assert.ok(Object.isFrozen(result));
  assert.ok(Object.isFrozen(result.nodes[1].props));
});
test('node count, graph depth and Unicode key boundaries', () => {
  const many = [node('root', 'Column', [], Array.from({ length: 4095 }, (_, i) => `n${i}`)), ...Array.from({ length: 4095 }, (_, i) => node(`n${i}`))];
  assert.equal(validateView(arena(many)).nodes.length, 4096);
  many[0].children.push('extra'); many.push(node('extra'));
  assert.throws(() => validateView(arena(many)), /node/i);
  const chain = Array.from({ length: 64 }, (_, i) => node(`n${i}`, 'Column', [], i < 63 ? [`n${i + 1}`] : []));
  assert.equal(validateView(arena(chain)).nodes.length, 64);
  chain[63].children.push('extra'); chain.push(node('extra'));
  assert.throws(() => validateView(arena(chain)), /depth/i);
  assert.equal(validateView(arena([node('😀'.repeat(128))])).nodes.length, 1);
  assert.throws(() => validateView(arena([node('😀'.repeat(129))])), /key/i);
});
test('rejects malformed graph without mutation', () => {
  const cases = [
    { ...arena(), schemaVersion: 2 }, arena([node('')]), arena([node('a'), node('a')]),
    arena([node('root')], 'missing'), arena([node('root', 'Column', [], ['missing'])]),
    arena([node('root', 'Column', [], ['child', 'child']), node('child')]),
    arena([node('root'), node('unreachable')]),
    arena([node('root', 'Column', [], ['child']), node('child', 'Column', [], ['root'])]),
    arena([node('root', 'Column', [], ['a', 'b']), node('a', 'Column', [], ['c']), node('b', 'Column', [], ['c']), node('c')]),
    arena([node('root', 'Script')]), arena([node('root', 'Column', [e('HTML', '<script>')])]),
    arena([node('root', 'Column', [e('Gap', e('Md')), e('Gap', e('Lg'))])]),
    arena([node('root', 'Column', [], ['child']), node('child', 'Card', [e('Theme', e('Dark'))])]),
  ];
  for (const value of cases) {
    const before = JSON.stringify(value);
    assert.throws(() => validateView(value), TypeError);
    assert.equal(JSON.stringify(value), before);
  }
});
test('semantic prop combinations are closed and precise', () => {
  for (const level of [1, 2, 3, 4, 5, 6]) validateView(arena([node('root', 'Heading', [e('Content', 'Title'), e('Level', level)])]));
  for (const variant of ['Primary', 'Secondary', 'Outline', 'Ghost', 'Destructive']) {
    const value = button('root'); value.props[1] = e('Variant', e(variant)); validateView(arena([value]));
  }
  const bad = [
    node('root', 'Heading', [e('Content', 'x'), e('Level', 7)]),
    node('root', 'Heading', [e('Content', 'x'), e('Level', 1.5)]),
    node('root', 'Text', []), node('root', 'Text', [e('Content', 3)]),
    node('root', 'Text', [e('Content', 'x'), e('Variant', e('Primary'))]),
    node('root', 'Column', [e('Pressed', true)]),
    { ...button('root'), props: button('root').props.slice(1) },
    { ...button('root'), props: button('root').props.map(p => p.tag === 'Content' ? e('Content', '   ') : p) },
    node('root', 'Badge', [e('Content', 'x'), e('Tone', e('Unknown'))]),
    node('root', 'Row', [e('Gap', e('Md', 2))]),
  ];
  for (const value of bad) assert.throws(() => validateView(arena([value])), TypeError);
  assert.throws(() => validateView(arena([{ ...button('root'), children: ['child'] }, button('child')])), /children|interactive/i);
  validateView(arena([{ ...button('root'), props: [...button('root').props, e('Pressed', true)] }]));
});
