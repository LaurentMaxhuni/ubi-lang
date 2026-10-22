import { test } from 'node:test';
import assert from 'node:assert/strict';
import { argument, validateView } from '../renderer.mjs';
const tag = (tag, ...values) => ({tag, values});
const node = (key = 'root', kind = 'Text', props = [], children = []) => ({key, kind:tag(kind), props, children});
const view = (...nodes) => ({schemaVersion:1, root:nodes[0].key, nodes});

test('public Ubi arguments pass scalars directly and encode only aggregates', () => {
  for (const scalar of [false, true, 0, 42, 'day', '', null]) assert.equal(argument(scalar), scalar);
  assert.deepEqual(JSON.parse(argument({ value: 'day' })), { value: 'day' });
  assert.deepEqual(JSON.parse(argument(['day'])), ['day']);
  assert.throws(() => argument(() => 1));
});

test('accepts schema-1 source ABI and all primitive kinds', () => {
  for (const kind of 'Window View Column Row Flex Grid ScrollView Card Text Heading Separator Button IconButton Checkbox Switch Slider TextInput PasswordInput Badge ProgressBar Radio Tab Toggle ToggleItem Link TextArea NumberInput Avatar Alert Kbd Skeleton Label'.split(' ')) {
    assert.equal(validateView(view(node('root', kind, kind === 'IconButton' ? [tag('Label','Close')] : []))).nodes[0].kind, kind);
  }
});
test('rejects malformed graph, unknown data, incompatible properties and numeric bounds', () => {
  const invalid = [
    view(node('', 'Text')), view(node('root','Unknown')), view(node('root','Text',[tag('Unknown',1)])),
    view(node('root','Text',[tag('Content','a'),tag('Content','b')])),
    view(node('root','Column',[],['root'])), view(node('root','Column',[],['missing'])),
    view(node('root'),node('unreachable')), view(node('root','Text',[],['leaf']),node('leaf')),
    view(node('root','Text',[tag('Width',1.2)])),view(node('root','Text',[tag('WidthPercent',101)])),
    view(node('root','Text',[tag('MinWidth',20),tag('MaxWidth',10)])),
    view(node('root','Text',[tag('Checked',true),tag('Selected',false)])),
    view(node('root','IconButton')), view(node('root','Text',[tag('Role','script')])),
    view(node('root','Text',[tag('Step',0)])),view(node('root','Text',[tag('Content','é'.repeat(8193))])),
    {...view(node('root')),extra:true},
  ];
  for (const raw of invalid) assert.throws(() => validateView(raw), TypeError);
});
test('rejects aliases, excessive depth, prototype records and accessors without executing them', () => {
  assert.throws(() => validateView(view(node('root','Column',[],['a','a']),node('a'))));
  const nodes = Array.from({length:34},(_,i)=>node(String(i),'Column',[],i<33?[String(i+1)]:[]));
  assert.throws(() => validateView(view(...nodes)));
  assert.throws(() => validateView(Object.assign(Object.create({}),view(node()))));
  let executed = false;
  const raw = view(node());
  Object.defineProperty(raw,'root',{get(){executed=true;return 'root';},enumerable:true});
  assert.throws(() => validateView(raw)); assert.equal(executed,false);
});
test('normalization is detached and messages use bounded codec validation', () => {
  const raw = view(node('root','Button',[tag('Activate',tag('Click'))]));
  const normalized = validateView(raw); raw.nodes[0].props[0].values[0].tag='Mutated';
  assert.equal(normalized.nodes[0].props.Activate.tag,'Click');
  const cycle={}; cycle.self=cycle;
  assert.throws(()=>validateView(view(node('root','Button',[tag('Activate',cycle)]))));
});
test('native defaults and source property order determine final layout', () => {
  const normalized=validateView(view(node('root','Window',[tag('WidthPercent',100),tag('Width',400),tag('Level',1),tag('FontSize',20)])));
  assert.deepEqual(normalized.nodes[0].style,{gap:24,padding:32,fontSize:20,columns:2,width:'400px'});
  assert.equal(validateView(view(node('root','Card'))).nodes[0].style.padding,24);
  assert.equal(validateView(view(node('root','Heading'))).nodes[0].style.fontSize,26);
  for(const props of [[tag('Absolute',-100001,0)],[tag('Columns',65)],[tag('FontSize',7)],[tag('FontSize',97)],[tag('Step',2147483648)]]) assert.throws(()=>validateView(view(node('root','View',props))));
});
