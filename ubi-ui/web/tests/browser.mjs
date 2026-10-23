// ABI fixtures exercise renderer failure paths. Production reducers remain Ubi.
import { mount } from '../renderer.mjs';
const output=document.querySelector('#results'), results=[];
const assert=(condition,message)=>{if(!condition)throw new Error(message);};
const tag=(tag,...values)=>({tag,values});
const node=(key,kind,props=[],children=[])=>({key,kind:tag(kind),props,children});
const view=(...nodes)=>({schemaVersion:1,root:nodes[0].key,nodes});
const prop=(name,value)=>tag(name,value);
const message=tag('Action');
async function test(name,fn) {
  const root=document.createElement('div');root.style.width='300px';document.body.append(root);
  try {await fn(root);results.push({name,ok:true});} catch(error) {results.push({name,ok:false,error:String(error.stack)});} finally {root.remove();}
}
await test('typed source ABI, retained input, IME, theme and disposal',root=>{
  const app={init:()=>'',update:(_,event)=>JSON.parse(event).values[1],view:model=>view(node('root','Window',[],['input']),node('input','TextInput',[prop('Label','Name'),prop('Value',model),prop('Input',message)]))};
  const handle=mount(root,app),input=root.querySelector('input'); input.focus(); input.value='Orbit';input.setSelectionRange(3,3);
  input.dispatchEvent(new InputEvent('input',{bubbles:true}));
  assert(handle.getModel()==='Orbit','input reducer');assert(root.querySelector('input')===input,'keyed identity');assert(document.activeElement===input&&input.selectionStart===3,'focus and selection');
  input.value='Orbit!';input.dispatchEvent(new InputEvent('input',{bubbles:true,isComposing:true}));assert(handle.getModel()==='Orbit','IME intermediate ignored');
  input.dispatchEvent(new CompositionEvent('compositionend',{bubbles:true}));assert(handle.getModel()==='Orbit!','IME committed');
  handle.setTheme('dark');assert(root.firstElementChild.dataset.ubiTheme==='dark','theme changed');
  const copy=handle.getView();copy.root='bad';assert(handle.getView().root==='root','view snapshot isolated');
  handle.dispose();input.value='stale';input.dispatchEvent(new InputEvent('input',{bubbles:true}));assert(handle.getModel()==='Orbit!','disposed listeners');
});
await test('validation failure and DOM patch failure preserve last model and UI',root=>{
  const errors=[];let invalid=true;
  const app={init:()=>0,update:model=>model+1,view:model=>{
    const n=model;if(n&&invalid)return view(node('root','Text',[prop('Unknown',true)]));
    return view(node('root','Window',[],n?['button','extra']:['button']),node('button','Button',[prop('Content',`Count ${n}`),prop('Activate',message)]),...(n?[node('extra','Text',[prop('Content','Added')])]:[]));
  }};
  const handle=mount(root,app,{onError:error=>errors.push(error)}),button=root.querySelector('button');button.focus();button.click();
  assert(handle.getModel()===0&&button.textContent==='Count 0'&&errors.length===1,'invalid view rolled back');
  invalid=false;const container=root.firstElementChild,insert=container.insertBefore;
  container.insertBefore=function(...args){container.insertBefore=insert;throw new Error('injected DOM failure');};
  button.click();assert(handle.getModel()===0&&button.textContent==='Count 0'&&document.activeElement===button&&errors.length===2,'DOM rollback');
  button.click();assert(handle.getModel()===1&&button.textContent==='Count 1'&&root.querySelectorAll('button').length===1,'working update after rollback');
  assert(root.querySelector('button')===button,'button identity');handle.dispose();
});
await test('checkbox, switch, slider and disabled bindings',root=>{
  let events=[];
  const app={init:()=>false,update:(_,event)=>{events.push(JSON.parse(event));return events.at(-1).values[1];},view:model=>view(node('root','Window',[],['check','switch','slider','disabled']),
    node('check','Checkbox',[prop('Label','Checkbox'),prop('Checked',model===true),prop('Toggle',message)]),
    node('switch','Switch',[prop('Label','Switch'),prop('Checked',model===true),prop('Toggle',message)]),
    node('slider','Slider',[prop('Label','Volume'),prop('Progress',40),prop('Change',message)]),
    node('disabled','Button',[prop('Content','Disabled'),prop('Disabled',true),prop('Activate',message)]))};
  const handle=mount(root,app);
  root.querySelector('input[type="checkbox"]').click();assert(events.at(-1).tag==='Toggle'&&handle.getModel()===true,'native checkbox');
  root.querySelector('[role="switch"]').click();assert(handle.getModel()===false,'native switch');
  const slider=root.querySelector('input[type="range"]');slider.value='63';slider.dispatchEvent(new Event('input',{bubbles:true}));assert(events.at(-1).tag==='Select'&&events.at(-1).values[1]==='63','slider Select');
  const count=events.length;root.querySelector('button').click();assert(events.length===count,'disabled ignored');handle.dispose();
});
await test('reordering keyed controls retains focus and scroll',root=>{
  const app={init:()=>false,update:()=>true,view:model=>view(node('root','Window',[],model?['b','a']:['a','b']),node('a','TextInput',[prop('Value','abc'),prop('Input',message)]),node('b','Text',[prop('Content','Other')]))};
  const handle=mount(root,app),input=root.querySelector('input');input.focus();input.setSelectionRange(1,1);input.dispatchEvent(new InputEvent('input',{bubbles:true}));
  assert(root.querySelector('input')===input&&document.activeElement===input&&input.selectionStart===1,'focus survives keyed move');handle.dispose();
});
await test('radio/tab arrow selection, toggle focus only and mount-scoped groups',root=>{
  for(const kind of ['Radio','Tab','ToggleItem']) {
    let calls=0;
    const app={init:()=> 'a',update:(_,event)=>{calls++;return JSON.parse(event).values[1];},view:model=>view(node('root','Row',[prop('Role',kind==='Radio'?'radiogroup':kind==='Tab'?'tablist':'group')],['a','b','c']),...['a','b','c'].map(key=>node(key,kind,[prop('Content',key),prop('Label',key),prop('Group','shared'),prop('Option',key),prop('Selected',model===key),prop('Disabled',key==='b'),prop('Change',message)])))};
    const handle=mount(root,app);const first=root.querySelector(kind==='Radio'?'input':'button');first.focus();first.dispatchEvent(new KeyboardEvent('keydown',{key:'ArrowRight',bubbles:true,cancelable:true}));
    assert(document.activeElement.closest('[data-ubi-key]').dataset.ubiKey==='c',`${kind} skips disabled`);
    assert(kind==='ToggleItem'?calls===0&&handle.getModel()==='a':calls===1&&handle.getModel()==='c',`${kind} correct selection behavior`);handle.dispose();
  }
});
await test('all primitive controls fit narrow mounts without changing host controls',root=>{
  const host=document.createElement('button');host.textContent='Host';document.body.append(host);const before=getComputedStyle(host).minHeight;
  const app={init:()=>0,update:()=>0,view:()=>view(node('root','Window',[],['input','button','grid']),node('input','TextInput',[prop('Label','Workspace'),prop('Width',460),prop('Value','Narrow')]),node('button','Button',[prop('Content','Button'),prop('Width',420)]),node('grid','Grid',[prop('Columns',3)],['a','b']),node('a','Text',[prop('Content','One')]),node('b','Text',[prop('Content','Two')]))};
  const handle=mount(root,app);assert(root.scrollWidth<=root.clientWidth,'narrow root overflow');assert(getComputedStyle(host).minHeight===before,'host CSS isolation');assert(root.querySelector('button').getBoundingClientRect().height>=44,'touch target');handle.dispose();host.remove();
});
await test('semantic feedback text has readable contrast in both themes',root=>{
  const luminance = rgb => {
    const values = rgb.match(/[\d.]+/g).slice(0,3).map(Number).map(value=>value/255).map(value=>value<=.04045?value/12.92:((value+.055)/1.055)**2.4);
    return values[0]*.2126+values[1]*.7152+values[2]*.0722;
  };
  for(const theme of ['light','dark']) for(const tone of ['Success','Warning','Danger']) {
    const app={init:()=>0,update:()=>0,view:()=>view(node('root','Window',[],['alert']),node('alert','Alert',[prop('Content','Status'),prop('Tone',tag(tone))]))};
    const handle=mount(root,app,{theme});
    const style=getComputedStyle(root.querySelector('.ubi-alert'));
    const a=luminance(style.color),b=luminance(style.backgroundColor),ratio=(Math.max(a,b)+.05)/(Math.min(a,b)+.05);
    assert(ratio>=4.5,`${theme} ${tone} contrast ${ratio.toFixed(2)}`);handle.dispose();
  }
});
await test('simultaneous mounts isolate radio groups, state and themes',root=>{
  const first=document.createElement('div'),second=document.createElement('div');root.append(first,second);
  const app={init:()=> 'a',update:(_,event)=>JSON.parse(event).values[1],view:model=>view(
    node('root','Row',[prop('Role','radiogroup')],['a','b']),
    ...['a','b'].map(key=>node(key,'Radio',[prop('Label',key),prop('Group','same-group'),prop('Option',key),prop('Selected',model===key),prop('Change',message)]))
  )};
  const a=mount(first,app,{theme:'light'}),b=mount(second,app,{theme:'dark'});
  try {
    const aInputs=first.querySelectorAll('input'),bInputs=second.querySelectorAll('input');
    assert(aInputs[0].name!==bInputs[0].name,'radio names must be mount-scoped');
    aInputs[1].click();
    assert(a.getModel()==='b'&&b.getModel()==='a'&&bInputs[0].checked,'one mount must not uncheck another');
    a.setTheme('dark');b.setTheme('light');
    assert(first.querySelector('[data-ubi-key="root"]').dataset.ubiTheme==='dark'&&second.querySelector('[data-ubi-key="root"]').dataset.ubiTheme==='light','themes remain independent');
  } finally {a.dispose();b.dispose();}
});
function fakeClock() {
  let time=0, nextId=0;
  const callbacks=new Map();
  return {
    now:()=>time,
    requestFrame:callback=>{const id=++nextId;callbacks.set(id,callback);return id;},
    cancelFrame:id=>callbacks.delete(id),
    pending:()=>callbacks.size,
    step:next=>{time=next;const pending=[...callbacks.values()];callbacks.clear();for(const callback of pending)callback(time);},
  };
}
const linearMotion={durationMs:100,delayMs:0,easing:tag('Linear')};
await test('finite motion uses presentation frames and sleeps after settling',root=>{
  const clock=fakeClock();let updates=0,views=0;
  const app={init:()=>0,update:model=>{updates++;return model+1;},view:model=>{views++;return view(
    node('root','Window',[],['move','target']),node('move','Button',[prop('Content','Move'),prop('Activate',message)]),
    node('target','Text',[prop('Content','Moving'),prop('Motion',linearMotion),tag('Translate',model*100,0)])
  );}};
  const handle=mount(root,app,{motion:'full',clock});
  const target=root.querySelector('[data-ubi-key="target"]');
  root.querySelector('button').click();
  const calls=[updates,views];
  clock.step(25);
  assert(parseFloat(target.style.getPropertyValue('--ubi-motion-x'))===25,'quarter-frame presentation');
  for(let time=26;time<=150;time++)clock.step(time);
  assert(updates===calls[0]&&views===calls[1],'frames must not rerun update or view');
  assert(clock.pending()===0,'settled presentation must stop requesting frames');
  root.querySelector('button').click();clock.step(175);
  assert(clock.pending()>0,'new transition schedules a frame');
  handle.setMotion('reduced');
  assert(clock.pending()===0&&parseFloat(target.style.getPropertyValue('--ubi-motion-x'))===200,'reduced motion settles and cancels frames');
  handle.setMotion('full');root.querySelector('button').click();
  assert(clock.pending()>0,'full motion restarts for a later update');
  handle.dispose();assert(clock.pending()===0,'dispose cancels pending frames');
});
await test('a failed DOM commit preserves an in-flight animation',root=>{
  const clock=fakeClock(),errors=[];
  const app={init:()=>0,update:model=>model+1,view:model=>view(
    node('root','Window',[],['move','target',...(model>1?['extra']:[])]),
    node('move','Button',[prop('Content','Move'),prop('Activate',message)]),
    node('target','Text',[prop('Content','Moving'),prop('Motion',linearMotion),tag('Translate',model*100,0)]),
    ...(model>1?[node('extra','Text',[prop('Content','Extra')])]:[])
  )};
  const handle=mount(root,app,{motion:'full',clock,onError:error=>errors.push(error)});
  const button=root.querySelector('button'),target=root.querySelector('[data-ubi-key="target"]');
  button.focus();button.click();clock.step(25);
  const container=root.querySelector('[data-ubi-key="root"]'),insert=container.insertBefore;
  container.insertBefore=function(...args){container.insertBefore=insert;throw Error('injected animated patch failure');};
  button.click();
  assert(errors.length===1&&handle.getModel()===1&&document.activeElement===button,'logical state and focus roll back');
  clock.step(50);
  assert(parseFloat(target.style.getPropertyValue('--ubi-motion-x'))===50,'previous track continues on its original timeline');
  clock.step(150);assert(clock.pending()===0,'previous track still settles');
  handle.dispose();
});
await test('visual exits remove editable values and live identity',root=>{
  const clock=fakeClock();
  const app={init:()=>true,update:model=>!model,view:model=>view(
    node('root','Window',[],['toggle',...(model?['card']:[])]),
    node('toggle','Button',[prop('Content','Toggle'),prop('Activate',message)]),
    ...(model?[
      node('card','Column',[prop('Motion',linearMotion),prop('Exit',tag('Fade'))],['secret','caption']),
      node('secret','PasswordInput',[prop('Label','Secret'),prop('Value','private-value')]),
      node('caption','Text',[prop('Content','Visible caption')])
    ]:[])
  )};
  const handle=mount(root,app,{motion:'full',clock});
  root.querySelector('button').click();
  const exit=root.querySelector('.ubi-exit-layer');
  assert(exit&&exit.inert&&exit.getAttribute('aria-hidden')==='true','exit must be visual-only');
  assert(!exit.querySelector('input,[id],[data-ubi-key],[tabindex]'),'exit omits editable content and live identities');
  assert(!exit.innerHTML.includes('private-value'),'exit cannot retain password values');
  root.querySelector('button').click();
  assert(root.querySelectorAll('[data-ubi-key="card"]').length===1&&!root.querySelector('.ubi-exit-layer'),'resurrection keeps one live identity');
  handle.dispose();assert(clock.pending()===0,'exit disposal cancels frames');
});
output.textContent=results.map(result=>`${result.ok?'PASS':'FAIL'} ${result.name}${result.error?'\n'+result.error:''}`).join('\n');
document.body.dataset.testStatus=results.every(result=>result.ok)?'passed':'failed';
window.__ubiBrowserTests=results;
