import { mount } from '../renderer.mjs';

const output=document.querySelector('#results'),results=[];
const assert=(condition,message)=>{if(!condition)throw new Error(message);};
const prop=(tag,value)=>({tag,values:[value]});
const node=(key,kind,props=[],children=[])=>({key,kind:{tag:kind,values:[]},props,children});
const view=(...nodes)=>({schemaVersion:1,root:nodes[0].key,nodes});
async function test(name,run) {
  const root=document.createElement('div');document.body.append(root);
  let handle;
  try {handle=await run(root);results.push({name,ok:true});}
  catch(error) {results.push({name,ok:false,error:String(error.stack)});}
  finally {handle?.dispose();root.remove();}
}
function appFor(kind) {
  const request=kind==='Tooltip'?'open-tooltip':'open-context';
  const action=kind==='ContextMenu'?'open-item':'button-action';
  const item=kind==='ContextMenu'?[node('item','MenuItem',[prop('Label','Open item'),prop('Option','open'),prop('Activate',action)])]:[];
  const app={
    init:()=>false,
    update:(model,event)=>{
      const decoded=JSON.parse(event);
      return decoded.tag==='Activate'&&decoded.values[0]===request?true:model;
    },
    view:model=>view(
      node('root','Window',[],['anchor','overlay']),
      node('anchor','Button',[prop('Content','Open '+kind),prop('Activate','button-action')]),
      node('overlay','Overlay',[
        prop('OverlayKind',{tag:kind,values:[]}),prop('Anchor','anchor'),prop('Open',model),prop('OpenRequest',request),prop('Label',kind==='Tooltip'?'Help text':'Actions'),
      ],item.map(child=>child.key)),
      ...item,
    ),
  };
  return {app,request};
}

await test('Tooltip focus OpenRequest passes through mount and updates source state',root=>{
  const {app}=appFor('Tooltip'),handle=mount(root,app),anchor=root.querySelector('[data-ubi-key="anchor"]'),overlay=root.querySelector('[data-ubi-key="overlay"]');
  anchor.focus();
  assert(handle.getModel()===true,'host focus request was rejected by renderer dispatch');
  assert(!overlay.hidden,'Tooltip did not open');
  assert(anchor.getAttribute('aria-describedby')?.includes(overlay.id),'open Tooltip description was not linked to its anchor');
  return handle;
});

await test('ContextMenu Shift+F10 OpenRequest passes through mount and focuses its item',root=>{
  const {app}=appFor('ContextMenu'),handle=mount(root,app),anchor=root.querySelector('[data-ubi-key="anchor"]');
  anchor.focus();anchor.dispatchEvent(new KeyboardEvent('keydown',{key:'F10',shiftKey:true,bubbles:true,cancelable:true}));
  assert(handle.getModel()===true,'host context request was rejected by renderer dispatch');
  assert(document.activeElement===root.querySelector('[data-ubi-key="item"]'),'ContextMenu did not move focus to its first item');
  return handle;
});

await test('OpenRequest cannot open from a hidden disabled anchor',root=>{
  let handle;
  const app={
    init:()=>false,update:(_,event)=>{const decoded=JSON.parse(event);return decoded.tag==='Activate'?true:false;},
    view:model=>view(
      node('root','Window',[],['closed','overlay']),
      node('closed','Column',[prop('Disclosure',false)],['anchor']),
      node('anchor','Button',[prop('Content','Hidden'),prop('Activate','button-action'),prop('Disabled',true)]),
      node('overlay','Overlay',[prop('OverlayKind',{tag:'Tooltip',values:[]}),prop('Anchor','anchor'),prop('Open',model),prop('OpenRequest','open-tooltip'),prop('Label','Hidden anchor')]),
    ),
  };
  handle=mount(root,app);
  root.querySelector('[data-ubi-key="anchor"]').dispatchEvent(new FocusEvent('focusin',{bubbles:true}));
  assert(handle.getModel()===false,'hidden/disabled anchor incorrectly dispatched OpenRequest');
  return handle;
});

await test('centered overlay keeps placement while Enter Scale interpolates transform',root=>{
  let now=0,nextFrame=0;
  const frames=new Map(),clock={now:()=>now,requestFrame(callback){const id=++nextFrame;frames.set(id,callback);return id;},cancelFrame(id){frames.delete(id);}};
  const app={
    init:()=>false,
    update:(_,event)=>JSON.parse(event).tag==='Activate',
    view:model=>view(
      node('window','Window',[],['trigger','dialog']),
      node('trigger','Button',[prop('Content','Open dialog'),prop('Activate','open')]),
      node('dialog','Overlay',[
        prop('OverlayKind',{tag:'Dialog',values:[]}),prop('Open',model),prop('Label','Motion dialog'),
        prop('Motion',{durationMs:200,delayMs:0,easing:{tag:'Linear',values:[]}}),prop('Enter',{tag:'Scale',values:[]}),
      ]),
    ),
  };
  const handle=mount(root,app,{motion:'full',clock}),dialog=root.querySelector('[data-ubi-key="dialog"]');
  root.querySelector('[data-ubi-key="trigger"]').click();
  const initial=getComputedStyle(dialog),start=initial.transform;
  assert(initial.translate.includes('-50%'),'center placement was not kept on the individual translate property');
  now=100;const pending=[...frames.values()];frames.clear();for(const callback of pending)callback(now);
  const middle=getComputedStyle(dialog).transform;
  assert(start!==middle,'overlay presentation transform did not interpolate');
  assert(getComputedStyle(dialog).translate.includes('-50%'),'motion replaced the modal centering offset');
  return handle;
});

output.dataset.failed=String(results.filter(result=>!result.ok).length);
output.textContent=results.map(result=>`${result.ok?'PASS':'FAIL'} ${result.name}${result.error?`\n${result.error}`:''}`).join('\n');
if(results.some(result=>!result.ok))throw new Error('Renderer OpenRequest checks failed');
