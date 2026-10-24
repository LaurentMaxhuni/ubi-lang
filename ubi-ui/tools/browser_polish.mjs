const source=new URLSearchParams(location.search).get('runtime')==='source';
if(source)document.querySelector('#runtime-css').href='../web/renderer.css';
const {mount}=await import(source?'../web/renderer.mjs':'../docs/public/previews/runtime/renderer.mjs');
const output=document.querySelector('#results'),fixtures=document.querySelector('#fixtures'),results=[];
Object.assign(fixtures.style,{position:'fixed',top:'80px',left:'16px',width:'calc(100vw - 32px)',zIndex:'20000'});
const tag=(tag,...values)=>({tag,values}),prop=(name,value)=>tag(name,value),node=(key,kind,props=[],children=[])=>({key,kind:tag(kind),props,children});
const view=(...nodes)=>({schemaVersion:1,root:nodes[0].key,nodes});
const byKey=(root,key)=>root.querySelector(`[data-ubi-key="${key}"]`),assert=(value,message)=>{if(!value)throw Error(message);};
function contrast(a,b){const luminance=color=>{const values=color.match(/[\d.]+/g).slice(0,3).map(Number).map(x=>{x/=255;return x<=.04045?x/12.92:((x+.055)/1.055)**2.4;});return values[0]*.2126+values[1]*.7152+values[2]*.0722;};const x=luminance(a),y=luminance(b);return (Math.max(x,y)+.05)/(Math.min(x,y)+.05);}
async function test(name,run){const root=document.createElement('div');root.style.marginTop='64px';fixtures.append(root);try{await run(root);results.push({name,ok:true});}catch(error){results.push({name,ok:false,error:error.stack});}finally{root.remove();}output.textContent=results.map(r=>`${r.ok?'PASS':'FAIL'} ${r.name}${r.error?'\n'+r.error:''}`).join('\n');}
for(const width of [320,768,1440])for(const theme of ['light','dark']) {
  await test(`${width}px ${theme}: opaque themed modal and readable command content`,root=>{
    root.style.width=`${width}px`;
    const app={init:()=>false,update:(_,event)=>JSON.parse(event).values[0]==='open',view:open=>view(
      node('root','Window',[],['trigger','palette']),node('trigger','Button',[prop('Content','Commands'),prop('Activate','open')]),
      node('palette','Overlay',[prop('OverlayKind',tag('CommandPalette')),prop('Label','Commands'),prop('Open',open),prop('Dismiss','close')],['search','command']),
      node('search','TextInput',[prop('Label','Search commands'),prop('Value',''),prop('Input','input')]),node('command','MenuItem',[prop('Content','Open settings'),prop('Label','Open settings'),prop('Option','settings'),prop('Activate','settings')])
    )};
    const handle=mount(root,app,{theme,motion:'reduced'});byKey(root,'trigger').click();
    const panel=byKey(root,'palette'),style=getComputedStyle(panel),command=getComputedStyle(byKey(root,'command'));
    assert(style.backgroundColor!=='rgba(0, 0, 0, 0)'&&style.getPropertyValue('--ubi-bg').trim(), 'portal lost its opaque theme');
    assert(contrast(command.color,style.backgroundColor)>=4.5,'command text contrast below AA');
    assert(panel.getBoundingClientRect().width<=width,'modal exceeds mount width');
    assert(byKey(root,'command').getBoundingClientRect().height>=44,'command action has no usable target');
    handle.setTheme(theme==='light'?'dark':'light');assert(root.dataset.ubiTheme!==theme,'portal theme did not update');
    handle.dispose();
  });
  await test(`${width}px ${theme}: anchored dropdown does not cover its trigger`,root=>{
    root.style.width=`${width}px`;
    const app={init:()=>false,update:()=>true,view:open=>view(
      node('root','Window',[prop('Padding',24)],['trigger','menu']),node('trigger','Button',[prop('Content','Open menu'),prop('Activate','open')]),
      node('menu','Overlay',[prop('OverlayKind',tag('DropdownMenu')),prop('Anchor','trigger'),prop('Placement',tag('Bottom')),prop('Label','Actions'),prop('Open',open)],['option']),
      node('option','OptionItem',[prop('Content','Create project'),prop('Label','Create project'),prop('Option','create'),prop('Selected',true),prop('Change','create')])
    )};
    const handle=mount(root,app,{theme,motion:'reduced'});byKey(root,'trigger').click();
    const anchor=byKey(root,'trigger').getBoundingClientRect(),menu=byKey(root,'menu').getBoundingClientRect();
    assert(menu.top>=anchor.bottom+6||menu.bottom<=anchor.top-6,`menu overlaps anchor: ${JSON.stringify({anchor:{top:anchor.top,bottom:anchor.bottom},menu:{top:menu.top,bottom:menu.bottom}})}`);
    const selected=getComputedStyle(byKey(root,'option'));assert(contrast(selected.color,selected.backgroundColor)>=4.5,'selected option text contrast below AA');
    handle.dispose();
  });
  await test(`${width}px ${theme}: custom controls, number steps and multiselect`,root=>{
    root.style.width=`${width}px`;
    const app={init:()=> '12',update:(_,event)=>JSON.parse(event).values[1],view:value=>view(
      node('root','Window',[],['number','check','radio','switch','slider','group']),
      node('number','NumberInput',[prop('Label','Seats'),prop('Value',value),prop('Min',0),prop('Max',100),prop('Step',2),prop('Input','number')]),
      node('check','Checkbox',[prop('Label','Email'),prop('Checked',true),prop('Toggle','check')]),
      node('radio','Radio',[prop('Label','Daily'),prop('Group','radio'),prop('Option','day'),prop('Selected',true),prop('Change','radio')]),
      node('switch','Switch',[prop('Label','Enabled'),prop('Checked',true),prop('Toggle','switch')]),
      node('slider','Slider',[prop('Label','Volume'),prop('Progress',60),prop('Change','slider')]),
      node('group','Row',[prop('Role','group')],['a','b']),...['a','b'].map(key=>node(key,'ToggleItem',[prop('Label',key),prop('Content',key),prop('Group','group'),prop('Option',key),prop('Selected',true),prop('Change','choice')]))
    )};
    const handle=mount(root,app,{theme,motion:'reduced'});
    for(const key of ['check','radio','slider'])assert(getComputedStyle(byKey(root,key).querySelector('input')).appearance==='none',`${key} still uses unstyled native chrome`);
    assert(getComputedStyle(byKey(root,'switch').querySelector('input')).cursor==='pointer','switch cursor');
    assert(!byKey(root,'group').classList.contains('ubi-segmented'),'multi-select treated as an exclusive segmented control');
    byKey(root,'number').querySelector('[data-ubi-number-step="1"]').click();assert(handle.getModel()==='14','number step did not update typed source value');
    byKey(root,'number').querySelector('[data-ubi-number-step="-1"]').click();assert(handle.getModel()==='12','number decrement');
    handle.dispose();
  });
}
await test('spotlights leave no central highlight after pointer exit',async root=>{
  root.style.width='320px';
  const app={init:()=>0,update:()=>0,view:()=>view(node('root','Window',[],['spot','grid']),...['spot','grid'].map(key=>node(key,'EffectSurface',[prop('Effect',tag(key==='spot'?'SpotlightCard':'GridBackground')),prop('Padding',24)],[])))};
  const handle=mount(root,app,{motion:'full'});
  for(const key of ['spot','grid']){
    const el=byKey(root,key);
    await new Promise(resolve=>{const observer=new IntersectionObserver(entries=>{if(entries.some(entry=>entry.isIntersecting)){observer.disconnect();requestAnimationFrame(resolve);}});observer.observe(el);});
    const r=el.getBoundingClientRect();el.dispatchEvent(new PointerEvent('pointermove',{bubbles:true,pointerType:'mouse',clientX:r.left+5,clientY:r.top+5}));
    assert(el.style.getPropertyValue('--ubi-spotlight-alpha')==='1','hover highlight absent on a visible surface');
    el.dispatchEvent(new PointerEvent('pointerout',{bubbles:true,relatedTarget:document.body}));assert(el.style.getPropertyValue('--ubi-spotlight-alpha')==='0','highlight stayed in center after leaving');
  }
  handle.dispose();
});
await test('visible rotating text advances automatically and pauses',async root=>{
  const app={init:()=>({index:0,paused:false}),update:(encoded,event)=>{const model=JSON.parse(encoded);return JSON.parse(event).values[0]==='pause'?{...model,paused:!model.paused}:{...model,index:1-model.index};},view:encoded=>{const model=JSON.parse(encoded);return view(
    node('root','Window',[],['phrases','pause']),
    node('phrases','EffectSurface',[prop('Effect',tag('RotatingText')),prop('Index',model.index),prop('AutoPlay',true),prop('PeriodMs',250),prop('Paused',model.paused),prop('Advance','advance')],['first','second']),
    node('first','Text',[prop('Content','Build beautifully'),prop('Selected',model.index===0)]),node('second','Text',[prop('Content','Ship confidently'),prop('Selected',model.index===1)]),
    node('pause','Button',[prop('Content','Pause'),prop('Activate','pause')])
  );}};
  const handle=mount(root,app,{motion:'full'});
  try{
    await new Promise((resolve,reject)=>{
      const timeout=setTimeout(()=>{observer.disconnect();reject(Error('visible text never advanced'));},2000);
      const observer=new MutationObserver(()=>{if(handle.getModel().index===1){clearTimeout(timeout);observer.disconnect();resolve();}});
      observer.observe(root,{subtree:true,attributes:true,childList:true});
    });
    byKey(root,'pause').click();const index=handle.getModel().index;
    await new Promise(resolve=>setTimeout(resolve,300));
    assert(handle.getModel().index===index&&handle.getModel().paused,'paused text kept rotating');
  }finally{handle.dispose();}
});
document.body.dataset.testStatus=results.every(r=>r.ok)?'passed':'failed';
output.textContent+=`\n${results.filter(r=>r.ok).length}/${results.length} refinement checks passed`;
