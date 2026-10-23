import test from 'node:test';
import assert from 'node:assert/strict';
import { createEffects } from '../effects.mjs';

function fixture({ intersection = false } = {}) {
  let nextTimer = 0;
  let now = 0;
  const timers = new Map();
  let onIntersection = null;
  const media = { matches: false, addEventListener() {}, removeEventListener() {} };
  const win = {
    performance: { now: () => now },
    matchMedia: () => media,
    setTimeout(callback, delay) { const id = ++nextTimer; timers.set(id, { callback, delay }); return id; },
    clearTimeout(id) { timers.delete(id); },
  };
  if (intersection) win.IntersectionObserver = class {
    constructor(callback) { onIntersection = callback; }
    observe() {}
    unobserve() {}
    disconnect() {}
  };
  const doc = {
    visibilityState: 'visible', defaultView: win,
    addEventListener() {}, removeEventListener() {},
  };
  const listeners=new Map();
  const root = {
    nodeType: 1, ownerDocument: doc, setAttribute() {},
    addEventListener(name,listener) { const list=listeners.get(name)??[];list.push(listener);listeners.set(name,list); },
    removeEventListener(name,listener) { listeners.set(name,(listeners.get(name)??[]).filter(item=>item!==listener)); },
    dispatch(name,event) { for(const listener of listeners.get(name)??[])listener(event); },
  };
  const classes = new Set();
  const attrs = new Map([['data-ubi-key', 'rotating']]);
  const element = {
    ownerDocument: doc, classList: {
      add(value) { classes.add(value); }, remove(value) { classes.delete(value); },
    },
    style: { setProperty() {} },
    toggleAttribute(name, enabled) { if (enabled) attrs.set(name, ''); else attrs.delete(name); },
    setAttribute(name, value = '') { attrs.set(name, value); },
    getAttribute(name) { return attrs.get(name) ?? null; },
    removeAttribute(name) { attrs.delete(name); },
    closest(selector) { return selector==='[data-ubi-key]'?this:null; },
    contains(target) { return target===this; },
  };
  const view = { nodes: [{ key: 'rotating', children: [], props: { Effect: 'RotatingText', AutoPlay: true, PeriodMs: 1000 } }] };
  const entries = new Map([['rotating', { element }]]);
  return {
    root, view, entries, timers,
    advance(ms) { now += ms; },
    run(id) { const timer = timers.get(id); timers.delete(id); timer?.callback(); },
    intersect(shown) { onIntersection?.([{ target: element, isIntersecting: shown, intersectionRect: { width: shown ? 100 : 0, height: shown ? 30 : 0 } }]); },
  };
}

test('rotation does not reschedule when its advance removes the component', () => {
  const f = fixture();
  let view = f.view;
  let dispatches = 0;
  let effects;
  const dispatch = () => {
    dispatches++;
    view = { nodes: [] };
    effects.commit(view, new Map(), 'full', dispatch);
  };
  effects = createEffects(f.root, { motion: 'full' });
  effects.commit(view, f.entries, 'full', dispatch);
  const [timerId] = f.timers.keys();

  assert.doesNotThrow(() => f.run(timerId));
  assert.equal(dispatches, 1);
  assert.equal(f.timers.size, 0);
  effects.dispose();
});

test('a queued rotation callback cannot act on a reopened generation', () => {
  const f = fixture();
  let view = f.view;
  let dispatches = 0;
  const dispatch = () => { dispatches++; };
  const effects = createEffects(f.root, { motion: 'full' });
  effects.commit(view, f.entries, 'full', dispatch);
  const [staleId] = f.timers.keys();
  const stale = f.timers.get(staleId).callback;

  view = { nodes: [] };
  effects.commit(view, new Map(), 'full', dispatch);
  view = f.view;
  effects.commit(view, f.entries, 'full', dispatch);
  const [currentId] = f.timers.keys();
  stale();

  assert.equal(dispatches, 0);
  assert.notEqual(currentId, staleId);
  assert.ok(f.timers.has(currentId));
  effects.dispose();
});

test('offscreen rotation preserves only visible time and resumes on intersection', () => {
  const f = fixture({ intersection: true });
  const effects = createEffects(f.root, { motion: 'full' });
  effects.commit(f.view, f.entries, 'full', () => {});
  assert.equal(f.timers.size, 0);

  f.intersect(true);
  const [firstId] = f.timers.keys();
  assert.equal(f.timers.get(firstId).delay, 1000);
  f.advance(400);
  f.intersect(false);
  assert.equal(f.timers.size, 0);
  f.advance(10000);
  f.intersect(true);
  const [resumedId] = f.timers.keys();

  assert.equal(f.timers.get(resumedId).delay, 600);
  effects.dispose();
  assert.equal(f.timers.size, 0);
});

test('RotatingText stops at zero duration and ignores an already queued advance',()=>{
  const f=fixture();let view=f.view,advances=0;
  const effects=createEffects(f.root,{motion:'full'}),dispatch=()=>advances++;
  effects.commit(view,f.entries,'full',dispatch);
  const [timerId]=f.timers.keys(),stale=f.timers.get(timerId).callback;
  view={nodes:[{...view.nodes[0],props:{...view.nodes[0].props,Motion:{durationMs:0,delayMs:0,easing:'EaseOut'}}}]};
  effects.commit(view,f.entries,'full',dispatch);
  assert.equal(f.timers.size,0);
  stale();
  assert.equal(advances,0);
  view={nodes:[{...view.nodes[0],props:{...view.nodes[0].props,Motion:{durationMs:180,delayMs:0,easing:'EaseOut'}}}]};
  effects.commit(view,f.entries,'full',dispatch);
  assert.equal(f.timers.size,1);
  effects.dispose();
});

test('RotatingText clears a hover pause after autoplay is disabled',()=>{
  const f=fixture(),effects=createEffects(f.root,{motion:'full'}),element=f.entries.get('rotating').element;
  const view=autoPlay=>({nodes:[{...f.view.nodes[0],props:{...f.view.nodes[0].props,AutoPlay:autoPlay}}]});
  effects.commit(view(true),f.entries,'full',()=>{});
  f.root.dispatch('pointerover',{target:element,relatedTarget:null});
  assert.equal(f.timers.size,0);
  effects.commit(view(false),f.entries,'full',()=>{});
  f.root.dispatch('pointerout',{target:element,relatedTarget:{closest(){return null;}}});
  effects.commit(view(true),f.entries,'full',()=>{});
  assert.equal(f.timers.size,1);
  effects.dispose();
});

test('selection effects restore children when a keyed surface changes effect', () => {
  const f = fixture();
  const makeElement = key => {
    const attrs = new Map([['data-ubi-key', key]]);
    return {
      hidden: false, inert: false, style: { setProperty() {}, removeProperty() {} },
      classList: { add() {}, remove() {} }, toggleAttribute() {},
      setAttribute(name, value) { attrs.set(name, String(value)); },
      getAttribute(name) { return attrs.get(name) ?? null; },
      removeAttribute(name) { attrs.delete(name); },
      animate() {},
    };
  };
  const surface = makeElement('surface'), first = makeElement('first'), second = makeElement('second');
  const entries = new Map([['surface', { element: surface }], ['first', { element: first }], ['second', { element: second }]]);
  const view = effect => ({ nodes: [
    { key: 'surface', children: ['first', 'second'], props: { Effect: effect, Index: 0 } },
    { key: 'first', children: [], props: {} }, { key: 'second', children: [], props: {} },
  ] });
  const effects = createEffects(f.root, { motion: 'reduced' });

  effects.commit(view('Carousel'), entries, 'reduced');
  assert.equal(second.hidden, true);
  assert.equal(second.inert, true);
  assert.equal(second.getAttribute('aria-hidden'), 'true');
  effects.commit(view('GridBackground'), entries, 'reduced');

  assert.equal(first.hidden, false);
  assert.equal(first.inert, false);
  assert.equal(first.getAttribute('aria-hidden'), null);
  assert.equal(second.hidden, false);
  assert.equal(second.inert, false);
  assert.equal(second.getAttribute('aria-hidden'), null);
  effects.dispose();
});

function visualFixture() {
  const animations=[];
  const makeElement=(document,localName='div')=>{
    const attrs=new Map(),classes=new Set(),children=[];
    const element={
      nodeType:1,ownerDocument:document,localName,children,childNodes:children,attrs,classes,style:{values:new Map(),setProperty(name,value){this.values.set(name,String(value));},removeProperty(name){this.values.delete(name);}},
      classList:{add(name){classes.add(name);},remove(name){classes.delete(name);},contains(name){return classes.has(name);}},
      setAttribute(name,value=''){attrs.set(name,String(value));},getAttribute(name){return attrs.get(name)??null;},removeAttribute(name){attrs.delete(name);},toggleAttribute(name,on){if(on)attrs.set(name,'');else attrs.delete(name);},
      append(...items){children.push(...items);},replaceChildren(...items){children.splice(0,children.length,...items);},
      animate(frames,timing){const handle={frames,timing,canceled:false,finished:false,cancel(){this.canceled=true;},finish(){this.finished=true;}};animations.push(handle);return handle;},
      rect:{left:0,top:0,width:100,height:20},getBoundingClientRect(){return this.rect;},
      addEventListener(){},removeEventListener(){},
    };
    Object.defineProperty(element,'textContent',{get(){return children.map(child=>child.textContent??'').join('');},set(value){children.splice(0,children.length,{nodeType:3,textContent:String(value)});}});
    return element;
  };
  const document={visibilityState:'visible',addEventListener(){},removeEventListener(){},defaultView:{performance:{now:()=>0},matchMedia:()=>({matches:false,addEventListener(){},removeEventListener(){}})}};
  document.createElement=tag=>makeElement(document,tag);
  document.createTextNode=text=>({nodeType:3,textContent:String(text)});
  const root=makeElement(document);
  return {root,document,makeElement:(tag)=>makeElement(document,tag),animations};
}

test('AnimatedList applies keyed FLIP deltas after the committed reorder', () => {
  const f=visualFixture(),list=f.makeElement(),first=f.makeElement(),second=f.makeElement();
  first.rect={left:0,top:0,width:100,height:20};second.rect={left:0,top:24,width:100,height:20};
  const entries=new Map([['list',{element:list}],['first',{element:first}],['second',{element:second}]]);
  const view=keys=>({nodes:[
    {key:'list',kind:'EffectSurface',children:keys,props:{Effect:'AnimatedList',Motion:{durationMs:200,delayMs:0,easing:'Linear'}}},
    {key:'first',kind:'Text',children:[],props:{}},{key:'second',kind:'Text',children:[],props:{}},
  ]});
  const effects=createEffects(f.root,{motion:'full'}),initial=view(['first','second']);
  effects.commit(initial,entries,'full',()=>{},effects.prepare(initial,entries));
  const reordered=view(['second','first']),prepared=effects.prepare(reordered,entries);
  second.rect={left:0,top:0,width:100,height:20};first.rect={left:0,top:24,width:100,height:20};
  effects.commit(reordered,entries,'full',()=>{},prepared);

  assert.equal(f.animations.length,2);
  assert.deepEqual(f.animations.map(item=>item.frames[0].translate),['0px 24px','0px -24px']);
  effects.setMotion('reduced');
  assert.ok(f.animations.every(animation=>animation.canceled));
  effects.dispose();
});

test('TextReveal keeps complete logical text and caps animated word runs per mount', () => {
  const f=visualFixture(),host=f.makeElement(),visual=f.makeElement('span'),semantic=f.makeElement('span');
  const content=`  ${Array.from({length:300},(_,index)=>`word${index}`).join(' ')}  `;
  semantic.textContent=content;
  const entry={element:host,textVisual:visual,semantic},entries=new Map([['reveal',{...entry}]]);
  const view={nodes:[{key:'reveal',kind:'EffectSurface',children:[],props:{Effect:'TextReveal',Content:content,Label:content,Motion:{durationMs:300,delayMs:0,easing:'EaseOut'}}}]};
  const effects=createEffects(f.root,{motion:'full'});
  effects.commit(view,entries,'full',()=>{},effects.prepare(view,entries));
  const wordRuns=visual.childNodes.filter(child=>child.className==='ubi-text-reveal-word');

  assert.equal(visual.textContent,content);
  assert.equal(semantic.textContent,content);
  assert.equal(wordRuns.length,255);
  assert.equal(visual.childNodes.filter(child=>child.className==='ubi-text-reveal-tail').length,1);
  assert.equal(f.animations.length,255);
  effects.setMotion('reduced');
  assert.ok(f.animations.every(animation=>animation.canceled));
  effects.dispose();
});

test('TextReveal settles active word animations when duration becomes zero without changing text',()=>{
  const f=visualFixture(),content='A continuous sentence stays available.',host=f.makeElement(),visual=f.makeElement('span'),semantic=f.makeElement('span');
  semantic.textContent=content;
  const entries=new Map([['reveal',{element:host,textVisual:visual,semantic}]]),makeView=durationMs=>({nodes:[{key:'reveal',kind:'EffectSurface',children:[],props:{Effect:'TextReveal',Content:content,Label:content,Motion:{durationMs,delayMs:0,easing:'EaseOut'}}}]});
  const effects=createEffects(f.root,{motion:'full'}),animated=makeView(300);
  effects.commit(animated,entries,'full',()=>{},effects.prepare(animated,entries));
  assert.ok(f.animations.length>0);
  const active=[...f.animations],staticView=makeView(0);
  effects.commit(staticView,entries,'full',()=>{},effects.prepare(staticView,entries));
  assert.ok(active.every(animation=>animation.finished&&animation.canceled));
  assert.equal(f.animations.length,active.length);
  assert.equal(visual.textContent,content);
  assert.equal(semantic.textContent,content);
  effects.dispose();
});

test('TextReveal shares a bounded visual-run budget across the mount', () => {
  const f=visualFixture(),content=Array.from({length:300},(_,index)=>`w${index}`).join(' '),entries=new Map();
  const nodes=Array.from({length:17},(_,index)=>{
    const key=`reveal-${index}`,host=f.makeElement(),visual=f.makeElement('span'),semantic=f.makeElement('span');
    semantic.textContent=content;entries.set(key,{element:host,textVisual:visual,semantic});
    return {key,kind:'EffectSurface',children:[],props:{Effect:'TextReveal',Content:content,Label:content,Motion:{durationMs:300,delayMs:0,easing:'EaseOut'}}};
  });
  const view={nodes},effects=createEffects(f.root,{motion:'full'});
  effects.commit(view,entries,'full',()=>{},effects.prepare(view,entries));
  const visualRuns=[...entries.values()].reduce((total,entry)=>total+entry.textVisual.childNodes.filter(child=>child.nodeType===1).length,0);
  assert.equal(visualRuns,4096);
  assert.ok(f.animations.length<=4096);
  assert.ok([...entries.values()].every(entry=>entry.semantic.textContent===content));
  effects.dispose();
});

test('Skeleton uses the static marker in reduced motion and clears it in full motion', () => {
  const f=visualFixture(),element=f.makeElement(),entries=new Map([['skeleton',{element}]]);
  const view={nodes:[{key:'skeleton',kind:'Skeleton',children:[],props:{}}]};
  const effects=createEffects(f.root,{motion:'full'});
  effects.commit(view,entries,'full');
  assert.equal(element.getAttribute('data-ubi-static'),null);
  effects.setMotion('reduced');
  assert.equal(element.getAttribute('data-ubi-static'),'');
  effects.setMotion('full');
  assert.equal(element.getAttribute('data-ubi-static'),null);
  effects.dispose();
});

test('pointer effects honor zero-duration motion and settle when reduced motion starts',()=>{
  const f=visualFixture(),handlers=new Map(),element=f.makeElement();
  f.root.addEventListener=(type,handler)=>handlers.set(type,handler);
  f.root.contains=target=>target===element;
  element.closest=()=>element;element.setAttribute('data-ubi-key','tilt');
  const entries=new Map([['tilt',{element}]]),view=durationMs=>({nodes:[{key:'tilt',kind:'EffectSurface',children:[],props:{Effect:'TiltCard',Motion:{durationMs,delayMs:0,easing:'EaseOut'}}}]}),effects=createEffects(f.root,{motion:'full'});
  effects.commit(view(0),entries,'full');
  handlers.get('pointermove')({target:element,pointerType:'mouse',clientX:0,clientY:0});
  assert.equal(element.style.values.get('--ubi-tilt-x'),'0deg');
  effects.commit(view(180),entries,'full');
  handlers.get('pointermove')({target:element,pointerType:'mouse',clientX:0,clientY:0});
  assert.equal(element.style.values.get('--ubi-tilt-x'),'6deg');
  effects.setMotion('reduced');
  assert.equal(element.style.values.get('--ubi-tilt-x'),'0deg');
  assert.equal(element.getAttribute('data-ubi-static'),'');
  effects.dispose();
});
