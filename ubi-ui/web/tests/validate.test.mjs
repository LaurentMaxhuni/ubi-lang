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

test('normalizes bounded motion and animation component ABI', () => {
  const motion={durationMs:220,delayMs:20,easing:tag('Spring')};
  const normalized=validateView(view(node('root','EffectSurface',[
    tag('Effect',tag('SpotlightCard')),tag('Motion',motion),tag('Opacity',80),tag('Translate',-8,12),tag('Scale',96),tag('Enter',tag('Rise')),
  ],['child']),node('child')));
  assert.deepEqual(normalized.nodes[0].props.Motion,{durationMs:220,delayMs:20,easing:'Spring'});
  assert.deepEqual(normalized.nodes[0].props.Translate,[-8,12]);
  assert.equal(normalized.nodes[0].props.Enter,'Rise');
  const anchored=validateView(view(node('root','Column',[],['overlay','trigger']),node('overlay','Overlay',[tag('OverlayKind',tag('Popover')),tag('Anchor','trigger')]),node('trigger','Button',[tag('Activate',tag('Open'))])));
  assert.equal(anchored.nodes[1].props.Placement,'Bottom');
  assert.equal(validateView(view(node('root','Pagination',[tag('Page',2),tag('PageCount',4)]))).nodes[0].props.Page,2);
  const rejected=[
    view(node('root','EffectSurface',[tag('Effect',tag('Shimmer'))])),
    view(node('root','Button',[tag('Effect',tag('TiltCard'))])),
    view(node('root','EffectSurface',[tag('Effect',tag('NumberTicker'))])),
    view(node('root','EffectSurface',[tag('Effect',tag('TextReveal')),tag('Number',3)])),
    view(node('root','EffectSurface',[tag('Effect',tag('RotatingText')),tag('Index',2)],['child']),node('child')),
    view(node('root','Pagination',[tag('Page',5),tag('PageCount',4)])),
    view(node('root','Text',[tag('Motion',{durationMs:2001,delayMs:0,easing:tag('Linear')})])),
    view(node('root','Text',[tag('Opacity',101)])),
  ];
  for(const raw of rejected) assert.throws(()=>validateView(raw),TypeError);
});

test('Disclosure is valid on source containers and rejected on leaves', () => {
  const normalized=validateView(view(node('root','Column',[tag('Disclosure',true)],['child']),node('child','Text')));
  assert.equal(normalized.nodes[0].props.Disclosure,true);
  assert.throws(()=>validateView(view(node('root','Text',[tag('Disclosure',false)]))),TypeError);
});

test('overlay references and accessible modal contracts validate before render', () => {
  const trigger=node('trigger','Button',[tag('Label','Open help'),tag('Activate',tag('Open'))]);
  const tooltip=node('tooltip','Overlay',[tag('OverlayKind',tag('Tooltip')),tag('Label','Help text'),tag('Anchor','trigger'),tag('OpenRequest',tag('Request')),tag('Dismiss',tag('Close'))]);
  assert.equal(validateView(view(node('root','Column',[],['trigger','tooltip']),trigger,tooltip)).nodes[2].props.Anchor,'trigger');
  for(const raw of [
    view(node('root','Overlay',[tag('OverlayKind',tag('Dialog')),tag('Open',true)])),
    view(node('root','Overlay',[tag('OverlayKind',tag('Popover')),tag('Anchor','missing')])),
    view(node('root','Overlay',[tag('OverlayKind',tag('Tooltip')),tag('Anchor','root'),tag('OpenRequest',tag('Request'))])),
    view(node('root','Overlay',[tag('OverlayKind',tag('Toast')),tag('TimeoutMs',60001)])),
    view(node('root','Text',[tag('LabelledBy','missing')])),
    view(node('root','Overlay',[tag('OverlayKind',tag('Dialog')),tag('Label','Settings'),tag('LabelledBy','root')]))
  ]) assert.throws(()=>validateView(raw),TypeError);
});

test('overlay validation matches native placement, anchor, tooltip and accessible-label rules', () => {
  const drawer=validateView(view(node('drawer','Overlay',[tag('OverlayKind',tag('Drawer')),tag('Label','Settings')])));
  assert.equal(drawer.nodes[0].props.Placement,'Right');
  const trigger=node('trigger','Button',[tag('Label','Help')]);
  const describedTooltip=node('tip','Overlay',[tag('OverlayKind',tag('Tooltip')),tag('Content','Additional help'),tag('Anchor','trigger')]);
  assert.doesNotThrow(()=>validateView(view(node('root','Column',[],['trigger','tip']),trigger,describedTooltip)));

  const cyclic=()=>validateView(view(
    node('root','Column',[],['a','b']),
    node('a','Overlay',[tag('OverlayKind',tag('Popover')),tag('Anchor','b')]),
    node('b','Overlay',[tag('OverlayKind',tag('Popover')),tag('Anchor','a')]),
  ));
  assert.throws(cyclic,TypeError);
  assert.throws(()=>validateView(view(node('root','Overlay',[tag('OverlayKind',tag('Dialog')),tag('Label','   ')]))),TypeError);
  assert.throws(()=>validateView(view(
    node('root','Column',[],['trigger','tip']),trigger,
    node('tip','Overlay',[tag('OverlayKind',tag('Tooltip')),tag('Content','Help'),tag('Anchor','missing')]),
  )),TypeError);
  assert.throws(()=>validateView(view(
    node('root','Column',[],['trigger','other','tip']),trigger,
    node('other','Button',[tag('Label','Other'),tag('DescribedBy','tip')]),
    node('tip','Overlay',[tag('OverlayKind',tag('Tooltip')),tag('Content','Help'),tag('Anchor','trigger')]),
  )),TypeError);
  assert.throws(()=>validateView(view(
    node('root','Column',[],['trigger','tip']),trigger,
    node('tip','Overlay',[tag('OverlayKind',tag('Tooltip')),tag('Content','Help'),tag('Anchor','trigger')],['child']),
    node('child','Button',[tag('Label','Action')]),
  )),TypeError);
});

test('table schema keeps sortable action contract and rejects duplicate keys or widths', () => {
  const table=()=>view(
    node('table','Table',[tag('Label','Invoices'),tag('Role','table')],['headers','r1']),
    node('headers','TableRow',[tag('Role','row')],['h1','h2']),
    node('h1','TableHeader',[tag('Label','Name'),tag('Content','Name'),tag('Option','name'),tag('Change',tag('Sort'))]),
    node('h2','TableHeader',[tag('Label','Amount'),tag('Content','Amount'),tag('Option','amount'),tag('Disabled',true)]),
    node('r1','TableRow',[tag('Role','row'),tag('Option','invoice-1')],['c1','c2']),
    node('c1','TableCell',[tag('Content','Acme'),tag('Option','name')]),
    node('c2','TableCell',[tag('Content','$18'),tag('Option','amount')]),
  );
  const normalized=validateView(table());
  assert.equal(normalized.nodes.find(item=>item.kind==='TableHeader').props.Change.tag,'Sort');
  assert.equal(normalized.nodes.find(item=>item.key==='h2').props.Change,undefined);
  const invalid=table(); invalid.nodes.find(item=>item.key==='h2').props[2]=tag('Option','name');
  assert.throws(()=>validateView(invalid),TypeError);
  const tooFew=table(); tooFew.nodes.find(item=>item.key==='r1').children.pop();
  assert.throws(()=>validateView(tooFew),TypeError);
});
