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
output.textContent=results.map(result=>`${result.ok?'PASS':'FAIL'} ${result.name}${result.error?'\n'+result.error:''}`).join('\n');
document.body.dataset.testStatus=results.every(result=>result.ok)?'passed':'failed';
window.__ubiBrowserTests=results;
