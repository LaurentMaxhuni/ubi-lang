import test from 'node:test';
import assert from 'node:assert/strict';
import { createOverlays } from '../overlays.mjs';

function fakeElement(document) {
  const attributes = new Map();
  const children = [];
  const listeners = new Map();
  const addListener = (type, handler, capture = false) => {
    const key = `${type}:${Boolean(typeof capture === 'object' ? capture.capture : capture)}`;
    if (!listeners.has(key)) listeners.set(key, new Set());
    listeners.get(key).add(handler);
  };
  const removeListener = (type, handler, capture = false) => listeners.get(`${type}:${Boolean(typeof capture === 'object' ? capture.capture : capture)}`)?.delete(handler);
  const element = {
    nodeType: 1, ownerDocument: document, children, childNodes: children,
    dataset: {}, style: {}, classList: { add() {} },
    append(...items) { for (const item of items) { item.remove(); item.parentElement = this; children.push(item); } },
    insertBefore(item, before) { item.remove(); item.parentElement = this; const index = children.indexOf(before); children.splice(index < 0 ? children.length : index, 0, item); },
    setAttribute(name, value) { attributes.set(name, String(value)); },
    removeAttribute(name) { attributes.delete(name); },
    getAttribute(name) { return attributes.get(name) ?? null; },
    addEventListener: addListener, removeEventListener: removeListener,
    dispatch(type, event = {}) { for (const capture of [true, false]) for (const handler of listeners.get(`${type}:${capture}`) ?? []) handler({ ...event, type, currentTarget: this }); },
    remove() { if (this.parentElement) { const index = this.parentElement.childNodes.indexOf(this); if (index >= 0) this.parentElement.childNodes.splice(index, 1); } this.parentElement = null; },
    contains(target) { return this === target || children.some(child => child.contains?.(target)); },
    closest(selector) {
      for (let node = this; node; node = node.parentElement) {
        if (selector.includes('[data-ubi-key]') && node.dataset.ubiKey) return node;
        if (selector.includes('[hidden]') && node.hidden) return node;
        if (selector.includes('[inert]') && node.inert) return node;
      }
      return null;
    },
    focus() { document.activeElement = this; },
    getBoundingClientRect() { return this.rect ?? { left: 0, top: 0, right: 80, bottom: 30, width: 80, height: 30 }; },
    querySelectorAll() { return []; }, getClientRects() { return []; },
  };
  Object.defineProperty(element, 'firstElementChild', { get: () => children[0] ?? null });
  Object.defineProperty(element, 'isConnected', { get() { let current = this; while (current.parentElement) current = current.parentElement; return current === document.root; } });
  return element;
}

function overlayFixture() {
  let nextTimer = 0;
  const timers = new Map();
  const win = {
    innerWidth: 800, innerHeight: 600,
    performance: { now: () => 0 },
    setTimeout(callback, delay) { const id = ++nextTimer; timers.set(id, { callback, delay }); return id; },
    clearTimeout(id) { timers.delete(id); },
    addEventListener() {}, removeEventListener() {},
  };
  const doc = fakeElement(null);
  Object.assign(doc, { defaultView: win, visibilityState: 'visible', activeElement: null });
  doc.createElement = () => fakeElement(doc);
  const root = fakeElement(doc);
  doc.root = root;
  const element = fakeElement(doc);
  const entries = new Map([['toast', { element }]]);
  const view = open => ({ nodes: [{ key: 'toast', kind: 'Overlay', children: [], props: { OverlayKind: 'Toast', Open: open, TimeoutMs: 5000 } }] });
  return { root, doc, win, entries, view, timers, makeElement: () => fakeElement(doc) };
}

test('opening a toast after a closed commit starts exactly one timeout', () => {
  const f = overlayFixture();
  const overlays = createOverlays(f.root);

  overlays.commit(f.view(false), f.entries);
  overlays.commit(f.view(true), f.entries);

  assert.equal(f.timers.size, 1);
  assert.equal([...f.timers.values()][0].delay, 5000);
  overlays.dispose();
  assert.equal(f.timers.size, 0);
});

test('tooltip dismisses when the pointer leaves its own surface', () => {
  const f = overlayFixture(), anchor = f.makeElement(), tooltip = f.makeElement(), outside = f.makeElement();
  anchor.dataset.ubiKey = 'anchor'; tooltip.dataset.ubiKey = 'tip';
  f.root.append(f.makeElement());
  const view = { nodes: [
    { key: 'anchor', kind: 'Button', children: [], props: {} },
    { key: 'tip', kind: 'Overlay', children: [], props: { OverlayKind: 'Tooltip', Anchor: 'anchor', Placement: 'Top', Open: true, Dismiss: 'close' } },
  ] };
  const entries = new Map([['anchor', { element: anchor }], ['tip', { element: tooltip }]]);
  const dismissed = [];
  const overlays = createOverlays(f.root);
  overlays.commit(view, entries, (node, prop) => dismissed.push([node.key, prop]));

  f.root.dispatch('pointerout', { target: tooltip, relatedTarget: outside });

  assert.deepEqual(dismissed, [['tip', 'Dismiss']]);
  overlays.dispose();
});

test('tooltip stays open while the pointer moves from its anchor onto the tooltip', () => {
  const f = overlayFixture(), main=f.makeElement(), anchor = f.makeElement(), tooltip = f.makeElement();
  anchor.dataset.ubiKey = 'anchor'; tooltip.dataset.ubiKey = 'tip';
  main.append(anchor); f.root.append(main);
  const view = { nodes: [
    { key: 'anchor', kind: 'Button', children: [], props: {} },
    { key: 'tip', kind: 'Overlay', children: [], props: { OverlayKind: 'Tooltip', Anchor: 'anchor', Placement: 'Top', Open: true, Dismiss: 'close' } },
  ] };
  const entries = new Map([['anchor', { element: anchor }], ['tip', { element: tooltip }]]);
  const dismissed=[];
  const overlays = createOverlays(f.root);
  overlays.commit(view, entries, (node, prop) => dismissed.push([node.key, prop]));

  f.root.dispatch('pointerout', { target: anchor, relatedTarget: tooltip });

  assert.deepEqual(dismissed, []);
  overlays.dispose();
});

test('removing an open modal restores focus before deleting its record', () => {
  const f = overlayFixture(), main = f.makeElement(), trigger = f.makeElement(), panel = f.makeElement();
  main.append(trigger); f.root.append(main); f.doc.activeElement = trigger;
  const openView = { nodes: [{ key: 'dialog', kind: 'Overlay', children: [], props: { OverlayKind: 'Dialog', Placement: 'Center', Open: true, Label: 'Dialog' } }] };
  const overlays = createOverlays(f.root);
  overlays.commit(openView, new Map([['dialog', { element: panel }]]));
  assert.equal(f.doc.activeElement, panel);

  overlays.commit({ nodes: [] }, new Map());

  assert.equal(f.doc.activeElement, trigger);
  assert.equal(panel.isConnected, false);
  overlays.dispose();
});

test('anchored overlays reposition on captured document scroll', () => {
  const f = overlayFixture(), main = f.makeElement(), anchor = f.makeElement(), popover = f.makeElement();
  main.append(anchor); f.root.append(main); anchor.dataset.ubiKey = 'anchor';
  anchor.rect = { left: 100, top: 100, right: 140, bottom: 120, width: 40, height: 20 };
  popover.rect = { left: 0, top: 0, right: 100, bottom: 50, width: 100, height: 50 };
  const view = { nodes: [{ key: 'popover', kind: 'Overlay', children: [], props: { OverlayKind: 'Popover', Anchor: 'anchor', Placement: 'Bottom', Open: true, Label: 'Menu' } }] };
  const overlays = createOverlays(f.root);
  overlays.commit(view, new Map([['anchor', { element: anchor }], ['popover', { element: popover }]]));
  assert.equal(popover.style.top, '128px');

  anchor.rect = { left: 100, top: 300, right: 140, bottom: 320, width: 40, height: 20 };
  f.doc.dispatch('scroll', { target: main });

  assert.equal(popover.style.top, '328px');
  overlays.dispose();
});

test('menu typeahead focuses enabled matches and leaves option selection to activation', () => {
  const f=overlayFixture(),anchor=f.makeElement(),panel=f.makeElement();
  const items=[
    {key:'apple',kind:'MenuItem',children:[],props:{Label:'Apple',Option:'apple'}},
    {key:'apricot',kind:'MenuItem',children:[],props:{Label:'Apricot',Option:'apricot'}},
    {key:'blue-disabled',kind:'MenuItem',children:[],props:{Label:'Bluebird',Option:'blue',Disabled:true}},
    {key:'blue',kind:'MenuItem',children:[],props:{Label:'Blue',Option:'blue-enabled'}},
  ];
  const controls=new Map(items.map(item=>[item.key,f.makeElement()]));
  for(const control of controls.values())panel.append(control);
  f.root.append(anchor,panel);
  const entries=new Map([
    ['anchor',{element:anchor}],['menu',{element:panel}],
    ...[...controls].map(([key,control])=>[key,{element:control,control}]),
  ]);
  const makeView=()=>({nodes:[
    {key:'root',kind:'Column',children:['anchor','menu'],props:{}},
    {key:'anchor',kind:'Button',children:[],props:{}},
    {key:'menu',kind:'Overlay',children:items.map(item=>item.key),props:{OverlayKind:'DropdownMenu',Anchor:'anchor',Placement:'Bottom',Open:true}},
    ...items,
  ]});
  const overlays=createOverlays(f.root);
  overlays.commit(makeView(),entries);
  const event=key=>({key,prevented:false,preventDefault(){this.prevented=true;}});

  const first=event('a');
  assert.equal(overlays.handleMenuKeydown(first,items[0]),true);
  assert.equal(f.doc.activeElement,controls.get('apricot'));
  assert.equal(first.prevented,true);
  assert.equal([...f.timers.values()][0].delay,500);

  const repeat=event('a');
  assert.equal(overlays.handleMenuKeydown(repeat,items[1]),true);
  assert.equal(f.doc.activeElement,controls.get('apple'));
  const expiry=[...f.timers.values()][0].callback;
  expiry();

  const disabledSkip=event('b');
  assert.equal(overlays.handleMenuKeydown(disabledSkip,items[0]),true);
  assert.equal(f.doc.activeElement,controls.get('blue'));
  assert.equal(disabledSkip.prevented,true);
  assert.equal(overlays.handleMenuKeydown(event('x'),{key:'query',kind:'TextInput',props:{}}),false);
  overlays.dispose();
  assert.equal(f.timers.size,0);

  const optionFixture=overlayFixture(),optionAnchor=optionFixture.makeElement(),optionPanel=optionFixture.makeElement();
  const options=[
    {key:'alpha',kind:'OptionItem',children:[],props:{Label:'Alpha',Option:'alpha'}},
    {key:'beta',kind:'OptionItem',children:[],props:{Label:'Beta',Option:'beta'}},
  ];
  const optionControls=new Map(options.map(item=>[item.key,optionFixture.makeElement()]));
  for(const control of optionControls.values())optionPanel.append(control);
  optionFixture.root.append(optionAnchor,optionPanel);
  const optionEntries=new Map([
    ['anchor',{element:optionAnchor}],['select',{element:optionPanel}],
    ...[...optionControls].map(([key,control])=>[key,{element:control,control}]),
  ]);
  const optionView={nodes:[
    {key:'root',kind:'Column',children:['anchor','select'],props:{}},
    {key:'anchor',kind:'Button',children:[],props:{}},
    {key:'select',kind:'Overlay',children:options.map(item=>item.key),props:{OverlayKind:'Select',Anchor:'anchor',Placement:'Bottom',Open:true}},
    ...options,
  ]};
  const optionOverlays=createOverlays(optionFixture.root),optionKey=event('b');
  optionOverlays.commit(optionView,optionEntries);
  assert.equal(optionOverlays.handleMenuKeydown(optionKey,options[0]),true);
  assert.equal(optionFixture.doc.activeElement,optionControls.get('beta'));
  assert.equal(optionKey.prevented,true);
  assert.equal(optionOverlays.comboboxInputForOption(options[1]),null);
  optionOverlays.dispose();
});

test('open Combobox input navigates with active descendant and keeps query focus', () => {
  const f=overlayFixture(),input=f.makeElement(),unrelated=f.makeElement(),panel=f.makeElement(),anchor=f.makeElement();
  const options=[
    {key:'disabled-first',kind:'OptionItem',children:[],props:{Label:'Unavailable',Option:'unavailable',Disabled:true}},
    {key:'first',kind:'OptionItem',children:[],props:{Label:'First',Option:'first'}},
    {key:'last',kind:'OptionItem',children:[],props:{Label:'Last',Option:'last'}},
    {key:'disabled-last',kind:'OptionItem',children:[],props:{Label:'Unavailable tail',Option:'unavailable-tail',Busy:true}},
  ];
  const controls=new Map(options.map(item=>[item.key,f.makeElement()]));
  for(const [key,control] of controls)control.id=`option-${key}`;
  for(const control of controls.values())panel.append(control);
  f.root.append(input,unrelated,anchor,panel);
  const entries=new Map([
    ['query',{element:input,control:input}],['unrelated',{element:unrelated,control:unrelated}],['anchor',{element:anchor}],['combo',{element:panel}],
    ...[...controls].map(([key,control])=>[key,{element:control,control}]),
  ]);
  const view=open=>({nodes:[
    {key:'root',kind:'Column',children:['query','unrelated','anchor','combo'],props:{}},
    {key:'query',kind:'TextInput',children:[],props:{}},
    {key:'unrelated',kind:'TextInput',children:[],props:{}},
    {key:'anchor',kind:'Button',children:[],props:{}},
    {key:'combo',kind:'Overlay',children:options.map(item=>item.key),props:{OverlayKind:'Combobox',Anchor:'query',Placement:'Bottom',Open:open}},
    ...options,
  ]});
  const overlays=createOverlays(f.root),event=key=>({key,prevented:false,preventDefault(){this.prevented=true;}});
  overlays.commit(view(true),entries);
  input.focus();
  assert.equal(input.getAttribute('aria-activedescendant'),controls.get('first').id);
  const down=event('ArrowDown');
  assert.equal(overlays.handleComboboxInputKeydown(down,view(true).nodes[1]),true);
  assert.equal(input.getAttribute('aria-activedescendant'),controls.get('last').id);
  const up=event('ArrowUp');
  assert.equal(overlays.handleComboboxInputKeydown(up,view(true).nodes[1]),true);
  assert.equal(input.getAttribute('aria-activedescendant'),controls.get('first').id);
  assert.equal(overlays.handleComboboxInputKeydown(event('End'),view(true).nodes[1]),true);
  assert.equal(input.getAttribute('aria-activedescendant'),controls.get('last').id);
  assert.equal(overlays.handleComboboxInputKeydown(event('Home'),view(true).nodes[1]),true);
  assert.equal(input.getAttribute('aria-activedescendant'),controls.get('first').id);
  assert.equal(down.prevented&&up.prevented,true);
  assert.equal(overlays.comboboxInputForOption(options[1]),input);
  const selected=[];
  assert.equal(overlays.handleComboboxInputKeydown(event('Enter'),view(true).nodes[1],option=>selected.push(option.props.Option)),true);
  assert.deepEqual(selected,['first']);
  assert.equal(f.doc.activeElement,input);
  assert.equal(overlays.handleComboboxInputKeydown({...event('Enter'),isComposing:true},view(true).nodes[1],()=>selected.push('composing')),false);
  assert.equal(overlays.handleComboboxInputKeydown({...event('Enter'),keyCode:229},view(true).nodes[1],()=>selected.push('ime')),false);
  assert.equal(selected.length,1);
  assert.equal(overlays.handleComboboxInputKeydown(event('f'),view(true).nodes[1]),false);
  assert.equal(overlays.handleComboboxInputKeydown(event('ArrowDown'),view(true).nodes[2]),false);

  input.focus();overlays.commit(view(false),entries,undefined,input);
  assert.equal(input.getAttribute('aria-activedescendant'),null);
  assert.equal(overlays.handleComboboxInputKeydown(event('ArrowDown'),view(false).nodes[1]),false);
  assert.equal(f.doc.activeElement,input);
  overlays.dispose();
});

test('Select and menus focus choices on open and restore the source trigger on close or removal', () => {
  for(const kind of ['Select','DropdownMenu','ContextMenu','Popover']) {
    const f=overlayFixture(),main=f.makeElement(),trigger=f.makeElement(),outside=f.makeElement(),panel=f.makeElement();
    const options=kind==='Select'?
      [{key:'first',kind:'OptionItem',children:[],props:{Label:'First',Option:'first'}},{key:'selected',kind:'OptionItem',children:[],props:{Label:'Selected',Option:'selected',Selected:true}}]:
      kind==='Popover'?[{key:'first',kind:'Button',children:[],props:{Activate:'activate'}}]:
      [{key:'first',kind:'MenuItem',children:[],props:{Label:'First'}},{key:'next',kind:'MenuItem',children:[],props:{Label:'Next'}}];
    const controls=new Map(options.map(item=>[item.key,f.makeElement()]));
    for(const control of controls.values())panel.append(control);
    main.append(trigger,outside);f.root.append(main,panel);trigger.focus();
    const entries=new Map([['trigger',{element:trigger,control:trigger}],['outside',{element:outside,control:outside}],['surface',{element:panel}],...[...controls].map(([key,control])=>[key,{element:control,control}])]);
    const view=open=>({nodes:[
      {key:'root',kind:'Column',children:['trigger','outside','surface'],props:{}},
      {key:'trigger',kind:'Button',children:[],props:{}},{key:'outside',kind:'Button',children:[],props:{}},
      {key:'surface',kind:'Overlay',children:options.map(item=>item.key),props:{OverlayKind:kind,Anchor:'trigger',Placement:'Bottom',Open:open}},...options,
    ]});
    const overlays=createOverlays(f.root);
    overlays.commit(view(true),entries,undefined,trigger);
    assert.equal(f.doc.activeElement,kind==='Select'?controls.get('selected'):kind==='Popover'?trigger:controls.get('first'),`${kind} initial focus`);
    if(kind==='Popover')controls.get('first').focus();
    const focused=f.doc.activeElement;
    overlays.commit(view(false),entries,undefined,focused);
    assert.equal(f.doc.activeElement,trigger,`${kind} close restores trigger`);

    overlays.commit(view(true),entries,undefined,trigger);
    outside.focus();
    overlays.commit(view(false),entries,undefined,outside);
    assert.equal(f.doc.activeElement,outside,`${kind} close preserves outside focus`);

    overlays.commit(view(true),entries,undefined,trigger);
    if(kind==='Popover')controls.get('first').focus();
    const inside=f.doc.activeElement;
    overlays.commit({nodes:[]},new Map(),undefined,inside);
    assert.equal(f.doc.activeElement,trigger,`${kind} removal restores trigger`);
    overlays.dispose();
  }
});

test('Shift+F10 requests only the context menu attached to the focused source trigger', () => {
  const f=overlayFixture(),trigger=f.makeElement(),panel=f.makeElement(),item=f.makeElement();
  trigger.dataset.ubiKey='trigger';panel.append(item);f.root.append(trigger,panel);trigger.focus();
  const view=open=>({nodes:[
    {key:'root',kind:'Column',children:['trigger','menu'],props:{}},
    {key:'trigger',kind:'Button',children:[],props:{}},
    {key:'menu',kind:'Overlay',children:['item'],props:{OverlayKind:'ContextMenu',Anchor:'trigger',Placement:'Bottom',Open:open,OpenRequest:'open'}},
    {key:'item',kind:'MenuItem',children:[],props:{Label:'Open',Option:'open',Activate:'activate'}},
  ]});
  const entries=new Map([['trigger',{element:trigger,control:trigger}],['menu',{element:panel}],['item',{element:item,control:item}]]),requests=[];
  const overlays=createOverlays(f.root);overlays.commit(view(false),entries,(node,prop)=>requests.push([node.key,prop]));
  let prevented=false;f.root.dispatch('keydown',{key:'F10',shiftKey:true,target:trigger,preventDefault(){prevented=true;}});
  assert.equal(prevented,true);assert.deepEqual(requests,[['menu','OpenRequest']]);
  overlays.commit(view(true),entries,undefined,trigger);
  assert.equal(f.doc.activeElement,item);
  overlays.dispose();
});

test('ContextMenu keyboard reopen discards the previous pointer position',()=>{
  const f=overlayFixture(),trigger=f.makeElement(),panel=f.makeElement();
  trigger.dataset.ubiKey='trigger';trigger.rect={left:100,top:100,right:140,bottom:120,width:40,height:20};panel.rect={left:0,top:0,right:80,bottom:30,width:80,height:30};
  f.root.append(trigger,panel);
  const view=open=>({nodes:[
    {key:'root',kind:'Column',children:['trigger','menu'],props:{}},
    {key:'trigger',kind:'Button',children:[],props:{}},
    {key:'menu',kind:'Overlay',children:[],props:{OverlayKind:'ContextMenu',Anchor:'trigger',Placement:'Bottom',Open:open,OpenRequest:'open'}},
  ]});
  const entries=new Map([['trigger',{element:trigger,control:trigger}],['menu',{element:panel}]]),overlays=createOverlays(f.root);
  overlays.commit(view(false),entries,(node,prop)=>{});
  f.root.dispatch('contextmenu',{target:trigger,clientX:780,clientY:580,preventDefault(){}});
  overlays.commit(view(true),entries,undefined,trigger);
  assert.equal(panel.style.left,'712px');assert.equal(panel.style.top,'562px');
  overlays.commit(view(false),entries,undefined,trigger);
  f.root.dispatch('keydown',{target:trigger,key:'F10',shiftKey:true,preventDefault(){}});
  overlays.commit(view(true),entries,undefined,trigger);
  assert.equal(panel.style.left,'80px');assert.equal(panel.style.top,'128px');
  overlays.dispose();
});

test('modal centering preserves the presentation transform and clears positioning on close',()=>{
  const f=overlayFixture(),panel=f.makeElement();
  panel.style.transform='translate(0px, 8px) scale(0.96)';
  f.root.append(panel);
  const entries=new Map([['dialog',{element:panel}]]),view=open=>({nodes:[
    {key:'dialog',kind:'Overlay',children:[],props:{OverlayKind:'Dialog',Open:open,Label:'Preview'}},
  ]}),overlays=createOverlays(f.root);
  overlays.commit(view(true),entries);
  assert.equal(panel.style.translate,'-50% -50%');
  assert.equal(panel.style.transform,'translate(0px, 8px) scale(0.96)');
  overlays.commit(view(false),entries);
  assert.equal(panel.style.translate,'');
  assert.equal(panel.style.transform,'translate(0px, 8px) scale(0.96)');
  overlays.dispose();
});
