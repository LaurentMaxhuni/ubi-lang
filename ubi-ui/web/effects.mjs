const animatedEffects=new Set(['BorderBeam','AuroraBackground','GridBackground','Marquee','RotatingText','Shimmer','Skeleton']);
const selectionEffects=new Set(['Carousel','CardStack','RotatingText']);
const clamp=(value,min,max)=>Math.min(max,Math.max(min,value));
const effectOf=node=>node?.props.Effect??(node?.kind==='Skeleton'?'Skeleton':undefined);
const easingFor=value=>({Linear:'linear',EaseOut:'cubic-bezier(.16,1,.3,1)',EaseInOut:'ease-in-out',Spring:'cubic-bezier(.2,1.3,.5,1)'})[value?.tag??value]??'cubic-bezier(.16,1,.3,1)';

function cleanCopy(element) {
  let copy=element.cloneNode(true);
  if(copy.matches?.('input,textarea,select,button,a,[tabindex]')){const replacement=docFor(element).createElement('span');replacement.textContent=copy.textContent??'';copy=replacement;}
  copy.querySelectorAll('.ubi-tab-indicator,.ubi-exit-layer,.ubi-disclosure-exit').forEach(node=>node.remove());
  const all=[copy,...copy.querySelectorAll('*')];
  for(const node of all) {
    if(node.matches?.('input,textarea,select,button,a,[tabindex]')) {node.replaceWith(node.ownerDocument.createTextNode(node.textContent??''));continue;}
    for(const name of node.getAttributeNames?.()??[])if(name==='id'||name==='name'||name==='for'||name==='data-ubi-key'||name==='role'||name==='tabindex'||name.startsWith('aria-')||name.startsWith('on'))node.removeAttribute(name);
    if('inert' in node)node.inert=true;
  }
  copy.setAttribute('aria-hidden','true');copy.inert=true;
  return copy;
}
const docFor=element=>element.ownerDocument;

/** Browser-only visual treatments and visibility-aware ambient scheduling. */
export function createEffects(root,options={}) {
  if(!root||root.nodeType!==1)throw new TypeError('Ubi effects require a mount element');
  const doc=root.ownerDocument,win=doc.defaultView,media=win.matchMedia?.('(prefers-reduced-motion: reduce)');
  let mode=options.motion??'system',currentView=null,currentEntries=new Map(),dispatch=()=>{},disposed=false,hidden=doc.visibilityState==='hidden';
  const records=new Map(),visible=new Map(),revealed=new Map(),timers=new Map(),pauseReasons=new Map();
  const intersection=win.IntersectionObserver?new win.IntersectionObserver(entries=>{
    for(const item of entries){const key=item.target.getAttribute('data-ubi-key'),shown=item.isIntersecting&&item.intersectionRect.width>0&&item.intersectionRect.height>0;visible.set(key,shown);const node=getNode(key);if(shown&&node?.props.Effect==='ScrollReveal')reveal(key);updateAmbient(key);if(node?.props.Effect==='RotatingText')pauseTimer(node,'intersection',!shown);}
  },{root:null,threshold:0.01}):null;
  const now=()=>win.performance.now();
  const reduced=()=>mode==='reduced'||(mode==='system'&&Boolean(media?.matches));
  const getNode=key=>currentView?.nodes.find(node=>node.key===key);
  const isVisible=key=>intersection?visible.get(key)===true:true;
  function reveal(key) {
    const node=getNode(key),element=currentEntries.get(key)?.element;
    if(!node||node.props.Effect!=='ScrollReveal'||!element)return;
    revealed.set(key,node.props.Effect);element.classList.remove('ubi-scroll-reveal-pending');element.classList.add('ubi-scroll-reveal-visible');
  }
  function staticEffect(node) {
    return reduced()||hidden||node.props.Motion?.durationMs===0||!isVisible(node.key);
  }
  function updateAmbient(key) {
    const node=getNode(key),element=currentEntries.get(key)?.element;if(!node||!element)return;
    const effect=effectOf(node);
    if(animatedEffects.has(effect))element.toggleAttribute('data-ubi-static',staticEffect(node));
    if(effect==='Marquee') {
      const reasons=pauseReasons.get(key)??new Set();
      element.toggleAttribute('data-ubi-paused',node.props.Paused||staticEffect(node)||reasons.size>0);
      const record=records.get(key);if(record?.track){const width=record.track.scrollWidth/2,viewport=element.clientWidth;record.track.style.setProperty('--ubi-marquee-duration',`${clamp(width/30,3,120)}s`);}
    }
  }
  function cloneMarquee(node,element) {
    let record=records.get(node.key);
    if(!record?.track||!record?.copy){const track=doc.createElement('div'),copy=doc.createElement('div');track.className='ubi-marquee-track';copy.className='ubi-marquee-copy';copy.setAttribute('aria-hidden','true');copy.inert=true;record={...record,track,copy};records.set(node.key,record);}
    const {track,copy}=record;
    if(track.parentElement!==element){
      const children=[...element.children].filter(child=>child!==track);
      for(const child of children)track.append(child);
      copy.replaceChildren(...children.map(cleanCopy));
      track.append(copy);element.replaceChildren(track);
    }
  }
  function cancelAnimations(animations,finish=false) {
    for(const animation of animations??[])try{if(finish)animation?.finish?.();animation?.cancel?.();}catch{}
  }
  function applySelection(node,allowMotion=true) {
    const element=currentEntries.get(node.key)?.element;if(!element)return;
    const children=node.children.map(key=>({key,element:currentEntries.get(key)?.element})).filter(item=>item.element);
    const selected=node.props.Index??0;
    const record=records.get(node.key)??{};
    if(record.selectionEffect&&record.selectionEffect!==node.props.Effect)restoreSelection(node.key);
    record.selectionEffect=node.props.Effect;
    record.selectionStates??=new Map();
    const childKeys=new Set(children.map(child=>child.key));
    for(const [key,state] of record.selectionStates)if(!childKeys.has(key)){restoreSelectionState(key,state);record.selectionStates.delete(key);}
    for(const child of children)if(!record.selectionStates.has(child.key))record.selectionStates.set(child.key,{element:child.element,hidden:child.element.hidden,inert:child.element.inert,ariaHidden:child.element.getAttribute('aria-hidden')});
    records.set(node.key,record);
    record.selectionAnimations??=new Map();
    if(node.props.Effect==='Carousel'||node.props.Effect==='RotatingText') {
      children.forEach((child,index)=>{
        const active=index===selected;
        child.element.hidden=!active;child.element.inert=!active;
        if(active)child.element.removeAttribute('aria-hidden');else child.element.setAttribute('aria-hidden','true');
        if(record.selected!==selected||reduced()||!allowMotion) {
          cancelAnimations([record.selectionAnimations.get(child.key)],true);record.selectionAnimations.delete(child.key);
        }
        if(allowMotion&&active&&node.props.Effect==='Carousel'&&record?.selected!==selected&&!reduced()&&node.props.Motion?.durationMs!==0) {
          const motion=node.props.Motion??{},easing={Linear:'linear',EaseOut:'cubic-bezier(.16,1,.3,1)',EaseInOut:'ease-in-out',Spring:'cubic-bezier(.2,1.3,.5,1)'}[motion.easing]??'cubic-bezier(.16,1,.3,1)';
          try{const animation=child.element.animate?.([{opacity:0,translate:'0px 8px'},{opacity:1,translate:'0px 0px'}],{duration:motion.durationMs??220,delay:motion.delayMs??0,easing,fill:'both'});if(animation)record.selectionAnimations.set(child.key,animation);}catch{}
        }
      });
      record.selected=selected;
    } else if(node.props.Effect==='CardStack') {
      element.style.position='relative';
      const total=children.length;
      children.forEach((child,index)=>{
        const depth=(index-selected+total)%total,shown=depth<3;
        child.element.hidden=!shown;child.element.inert=depth!==0;
        if(depth===0)child.element.removeAttribute('aria-hidden');else child.element.setAttribute('aria-hidden','true');
        child.element.style.zIndex=String(3-depth);child.element.style.position=depth===0?'relative':'absolute';
        child.element.style.inset=depth===0?'auto':'0';child.element.style.setProperty('--ubi-effect-y',`${depth*10}px`);child.element.style.setProperty('--ubi-effect-scale',String(1-depth*.035));
      });
    }
  }
  function restoreSelectionState(key,state,entries=currentEntries) {
    const element=entries.get(key)?.element??state.element;if(!element)return;
    element.hidden=state.hidden;element.inert=state.inert;
    if(state.ariaHidden===null)element.removeAttribute('aria-hidden');else element.setAttribute('aria-hidden',state.ariaHidden);
  }
  function restoreSelection(key,entries=currentEntries) {
    const record=records.get(key);if(!record?.selectionStates)return;
    cancelAnimations(record.selectionAnimations?.values(),true);record.selectionAnimations?.clear();
    for(const [childKey,state] of record.selectionStates)restoreSelectionState(childKey,state,entries);
    record.selectionStates=null;record.selectionEffect=null;record.selected=undefined;
  }
  function restoreAnimatedList(key,entries=currentEntries) {
    const record=records.get(key);if(!record?.listManaged)return;
    cancelAnimations(record.listAnimations?.values(),true);
    record.listAnimations?.clear();record.listRects=null;record.listKeys=null;record.listElements=null;record.listMounted=false;record.listManaged=false;
  }
  function prepare(view,entries=currentEntries) {
    const lists=new Map();
    for(const node of view.nodes)if(node.props.Effect==='AnimatedList') {
      const prior=currentView?.nodes.find(item=>item.key===node.key&&item.props.Effect==='AnimatedList');
      const rects=new Map(),keys=new Set();
      for(const key of prior?.children??[]) {
        const element=entries.get(key)?.element,rect=element?.getBoundingClientRect?.();
        if(!element)continue;keys.add(key);
        if(rect)rects.set(key,{left:rect.left,top:rect.top});
      }
      lists.set(node.key,{rects,keys});
    }
    return {lists};
  }
  function applyAnimatedList(node,prepared,allowMotion=true) {
    const record=records.get(node.key)??{};
    const children=node.children.map(key=>({key,element:currentEntries.get(key)?.element})).filter(item=>item.element);
    const previousRects=prepared?.rects??record.listRects??new Map(),previousKeys=prepared?.keys??record.listKeys??new Set();
    record.listAnimations??=new Map();record.listElements=new Map();
    const rects=new Map(),keys=new Set(),motion=node.props.Motion??{};
    const duration=clamp(motion.durationMs??240,0,2000),delay=clamp(motion.delayMs??0,0,5000),easing=easingFor(motion.easing);
    const animate=allowMotion&&!reduced()&&!hidden&&duration>0;
    children.forEach((child,index)=>{
      keys.add(child.key);record.listElements.set(child.key,child.element);
      cancelAnimations([record.listAnimations.get(child.key)]);record.listAnimations.delete(child.key);
      const rect=child.element.getBoundingClientRect?.();
      if(rect)rects.set(child.key,{left:rect.left,top:rect.top});
      const previous=previousRects.get(child.key),stagger=delay+Math.min(index,12)*28;
      if(!animate||!previousKeys.has(child.key)||!previous||!rect||typeof child.element.animate!=='function')return;
      const x=previous.left-rect.left,y=previous.top-rect.top;
      if(Math.abs(x)<=.5&&Math.abs(y)<=.5)return;
      try{const animation=child.element.animate([{translate:`${x}px ${y}px`},{translate:'0px 0px'}],{duration,delay:stagger,easing,fill:'both'});if(animation)record.listAnimations.set(child.key,animation);}catch{}
    });
    record.listRects=rects;record.listKeys=keys;record.listMounted=true;record.listManaged=true;records.set(node.key,record);
  }
  function restoreTextReveal(key,entries=currentEntries,node=getNode(key)) {
    const record=records.get(key);if(!record?.textRevealElement)return;
    cancelAnimations(record.wordAnimations,true);
    const element=entries.get(key)?.textVisual??record.textRevealVisual;
    if(node&&element){element.replaceChildren(doc.createTextNode(String(node.props.Content??'')));}
    record.wordAnimations=null;record.wordRuns=0;record.textRevealContent=undefined;record.textRevealElement=null;record.textRevealVisual=null;
  }
  function applyTextReveal(node,entry,allowMotion=true,budget={runs:255,animations:255}) {
    const record=records.get(node.key)??{},content=String(node.props.Content??''),element=entry.element,visual=entry.textVisual;
    if(!visual)return;
    visual.classList.add('ubi-text-reveal-visual');element.classList.add('ubi-text-reveal');
    const motion=node.props.Motion??{},duration=clamp(motion.durationMs??420,0,2000);
    if(record.textRevealElement===element&&record.textRevealContent===content){
      if(duration===0||reduced()) {cancelAnimations(record.wordAnimations,true);record.wordAnimations?.splice(0);}
      budget.runs=Math.max(0,budget.runs-(record.wordRuns??0));budget.animations=Math.max(0,budget.animations-(record.wordAnimations?.length??0));return;
    }
    cancelAnimations(record.wordAnimations);
    visual.replaceChildren();
    const words=[],tokens=content.match(/\s+|[^\s]+/gu)??[];let wordCount=0,tail='';
    const totalWords=tokens.reduce((count,token)=>count+(/^\s+$/u.test(token)?0:1),0);
    const reserveTail=totalWords>Math.min(255,budget.runs);
    const runLimit=Math.max(0,Math.min(255,budget.runs-(reserveTail?1:0)));
    for(let index=0;index<tokens.length;index++) {
      const token=tokens[index];
      if(/^\s+$/u.test(token)){if(wordCount>=runLimit)tail+=token;else visual.append(doc.createTextNode(token));continue;}
      if(wordCount>=runLimit){tail+=token+tokens.slice(index+1).join('');break;}
      const word=doc.createElement('span');word.className='ubi-text-reveal-word';word.textContent=token;visual.append(word);words.push(word);wordCount++;
    }
    let tailRun=0;
    if(tail){if(budget.runs>wordCount){const remainder=doc.createElement('span');remainder.className='ubi-text-reveal-tail';remainder.textContent=tail;visual.append(remainder);tailRun=1;}else visual.append(doc.createTextNode(tail));}
    const runCount=wordCount+tailRun;budget.runs-=runCount;
    record.textRevealElement=element;record.textRevealVisual=visual;record.textRevealContent=content;record.wordAnimations=[];record.wordRuns=runCount;
    const delay=clamp(motion.delayMs??0,0,5000);
    const animationCount=allowMotion&&!reduced()&&!hidden&&duration>0?Math.min(words.length,budget.animations):0;
    if(animationCount)words.slice(0,animationCount).forEach((word,index)=>{
      try{const animation=word.animate?.([{opacity:0,translate:'0px 12px'},{opacity:1,translate:'0px 0px'}],{duration,delay:delay+Math.min(index,18)*36,easing:easingFor(motion.easing),fill:'both'});if(animation)record.wordAnimations.push(animation);}catch{}
    });
    budget.animations-=animationCount;
    records.set(node.key,record);
  }
  function stopTimer(key) {
    const timer=timers.get(key);if(!timer)return;
    if(timer.id!==null)win.clearTimeout(timer.id);timers.delete(key);
  }
  function pauseTimer(node,reason,on) {
    const reasons=pauseReasons.get(node.key)??new Set();
    if(on)reasons.add(reason);else reasons.delete(reason);
    if(reasons.size)pauseReasons.set(node.key,reasons);else pauseReasons.delete(node.key);
    const timer=timers.get(node.key);if(!timer)return;
    if(on&&timer.id!==null){timer.remaining=Math.max(0,timer.due-now());win.clearTimeout(timer.id);timer.id=null;}
    if(!on&&!reasons.size&&timer.id===null)startTimer(node,timer);
  }
  function startTimer(node,timer) {
    if(!node||timers.get(node.key)!==timer||node.props.Effect!=='RotatingText'||node.props.Motion?.durationMs===0||reduced()||hidden||node.props.Paused||!isVisible(node.key)||(pauseReasons.get(node.key)?.size??0))return;
    timer.due=now()+timer.remaining;const generation=timer.generation;
    timer.id=win.setTimeout(()=>{const live=timers.get(node.key),current=getNode(node.key);if(live!==timer||live.generation!==generation||!current)return;live.id=null;live.remaining=current.props.PeriodMs??3000;dispatch(current,'Advance');const next=getNode(node.key);if(timers.get(node.key)===live&&next?.props.Effect==='RotatingText')startTimer(next,live);},timer.remaining);
  }
  function syncRotation(node) {
    const current=timers.get(node.key),period=node.props.PeriodMs??3000;
    if(!node.props.AutoPlay||node.props.Paused||node.props.Effect!=='RotatingText'||node.props.Motion?.durationMs===0) {stopTimer(node.key);return;}
    if(current&&current.period===period)return;
    stopTimer(node.key);const timer={period,remaining:period,due:0,id:null,generation:(current?.generation??0)+1};timers.set(node.key,timer);startTimer(node,timer);
  }
  function nodeForTarget(target) {
    const keyed=target?.closest?.('[data-ubi-key]');return keyed?getNode(keyed.getAttribute('data-ubi-key')):null;
  }
  function pauseFromEvent(event,reason,on) {
    let node=nodeForTarget(event.target);while(node&&!['Marquee','RotatingText'].includes(node.props.Effect))node=currentView.nodes.find(parent=>parent.children.includes(node.key));
    if(node)pauseTimer(node,reason,on);
  }
  function onPointerMove(event) {
    if(event.pointerType==='touch'||reduced())return;
    const element=event.target?.closest?.('[data-ubi-effect]');if(!element||!root.contains(element))return;
    const node=getNode(element.getAttribute('data-ubi-key'));if(!node)return;
    const rect=element.getBoundingClientRect();if(rect.width<=0||rect.height<=0)return;
    const x=clamp((event.clientX-rect.left)/rect.width,0,1),y=clamp((event.clientY-rect.top)/rect.height,0,1);
    if(node.props.Effect==='SpotlightCard'||node.props.Effect==='GridBackground'){element.style.setProperty('--ubi-pointer-x',`${x*100}%`);element.style.setProperty('--ubi-pointer-y',`${y*100}%`);}
    if(node.props.Effect==='TiltCard'){element.style.setProperty('--ubi-tilt-x',`${-(y-.5)*12}deg`);element.style.setProperty('--ubi-tilt-y',`${(x-.5)*12}deg`);}
    if(node.props.Effect==='Dock'){
      for(const key of node.children){const child=currentEntries.get(key)?.element;if(!child)continue;const bounds=child.getBoundingClientRect(),distance=Math.abs(event.clientX-(bounds.left+bounds.width/2));child.style.setProperty('--ubi-dock-scale',String(1+Math.max(0,1-distance/160)*.35));}
    }
  }
  function effectForTarget(target) {
    let node=nodeForTarget(target);
    while(node&&!node.props.Effect)node=currentView.nodes.find(parent=>parent.children.includes(node.key));
    return node;
  }
  function onPointerOut(event) {
    const node=effectForTarget(event.target);if(!node)return;
    const element=currentEntries.get(node.key)?.element;
    if(element&&(!event.relatedTarget||!element.contains(event.relatedTarget))){element.style.setProperty('--ubi-tilt-x','0deg');element.style.setProperty('--ubi-tilt-y','0deg');element.style.setProperty('--ubi-pointer-x','50%');element.style.setProperty('--ubi-pointer-y','50%');for(const key of node.children){currentEntries.get(key)?.element.style.setProperty('--ubi-dock-scale','1');}}
  }
  function onFocus(event,on) {
    const node=effectForTarget(event.target);if(!node)return;
    if(node.props.Effect==='ScrollReveal'&&on)reveal(node.key);
    if(['Marquee','RotatingText'].includes(node.props.Effect))pauseTimer(node,'focus',on);
  }
  function onVisibility() {hidden=doc.visibilityState==='hidden';for(const node of currentView?.nodes??[])if(animatedEffects.has(effectOf(node))||effectOf(node)==='RotatingText')updateAmbient(node.key);for(const node of currentView?.nodes??[])if(effectOf(node)==='RotatingText'){const timer=timers.get(node.key);if(timer){if(hidden&&timer.id!==null){timer.remaining=Math.max(0,timer.due-now());win.clearTimeout(timer.id);timer.id=null;}else if(!hidden&&timer.id===null)startTimer(node,timer);}}}
  function commit(view,entries,nextMode=mode,onDispatch=dispatch,prepared=undefined,flags={}) {
    if(disposed)throw new TypeError('Ubi effects are disposed');
    const previousView=currentView,previousEntries=currentEntries,allowMotion=flags.rollback!==true;
    currentView=view;currentEntries=entries;dispatch=onDispatch;mode=nextMode;
    root.setAttribute('data-ubi-motion',mode);
    const nextByKey=new Map(view.nodes.map(item=>[item.key,item]));
    for(const [key,record] of records) {
      const node=nextByKey.get(key),effect=effectOf(node);
      if(record.selectionEffect&&(!node||node.props.Effect!==record.selectionEffect))restoreSelection(key,entries);
      if(record.listManaged&&effect!=='AnimatedList')restoreAnimatedList(key,entries);
      if(record.textRevealElement&&effect!=='TextReveal')restoreTextReveal(key,entries, node);
    }
    const active=new Set();
    const revealBudget={runs:4096,animations:4096};
    for(const node of view.nodes) {
      const effect=effectOf(node);if(!effect)continue;
      active.add(node.key);const element=entries.get(node.key)?.element;if(!element)continue;
      records.set(node.key,records.get(node.key)??{});
      if(['SpotlightCard','TiltCard','CardStack','Carousel','Dock'].includes(effect))element.classList.add('ubi-expressive-surface');
      if(animatedEffects.has(effect))element.toggleAttribute('data-ubi-static',staticEffect(node));
      if(effect==='Marquee')cloneMarquee(node,element);
      if(['Carousel','CardStack','RotatingText'].includes(effect))applySelection(node,allowMotion);
      if(effect==='AnimatedList')applyAnimatedList(node,allowMotion?prepared?.lists?.get(node.key):undefined,allowMotion);
      if(effect==='TextReveal')applyTextReveal(node,entries.get(node.key),allowMotion,revealBudget);
      if(effect==='ScrollReveal') {
        const wasRevealed=revealed.get(node.key)===effect;
        if(reduced()||node.props.Motion?.durationMs===0){reveal(node.key);}
        else if(!wasRevealed){element.classList.add('ubi-scroll-reveal-pending');element.classList.remove('ubi-scroll-reveal-visible');intersection?.observe(element);if(!intersection&&element.getBoundingClientRect().height>0)reveal(node.key);}
        else {element.classList.remove('ubi-scroll-reveal-pending');element.classList.add('ubi-scroll-reveal-visible');}
      }
      if(intersection&&animatedEffects.has(effect))intersection.observe(element);
      updateAmbient(node.key);
      syncRotation(node);
    }
    for(const [key,record] of [...records])if(!active.has(key)){
      restoreSelection(key,previousEntries);restoreAnimatedList(key,previousEntries);restoreTextReveal(key,previousEntries,previousView?.nodes.find(node=>node.key===key));intersection?.unobserve(previousEntries.get(key)?.element);record.track?.remove();revealed.delete(key);records.delete(key);stopTimer(key);pauseReasons.delete(key);
    }
    for(const node of previousView?.nodes??[])if(effectOf(node)&&!active.has(node.key)){intersection?.unobserve(previousEntries.get(node.key)?.element);revealed.delete(node.key);stopTimer(node.key);pauseReasons.delete(node.key);}
    for(const [key,timer] of timers){if(!active.has(key)||getNode(key)?.props.Effect!=='RotatingText')stopTimer(key);}
  }
  function setMotion(nextMode) {
    if(!['system','full','reduced'].includes(nextMode))throw new TypeError('Unsupported Ubi motion mode');
    mode=nextMode;root.setAttribute('data-ubi-motion',mode);
    for(const node of currentView?.nodes??[]){const effect=effectOf(node),record=records.get(node.key);if(effect==='ScrollReveal'&&reduced())reveal(node.key);if(reduced()){cancelAnimations(record?.wordAnimations,true);record?.wordAnimations?.splice(0);cancelAnimations(record?.listAnimations?.values(),true);record?.listAnimations?.clear();cancelAnimations(record?.selectionAnimations?.values(),true);record?.selectionAnimations?.clear();}updateAmbient(node.key);if(effect==='RotatingText'){const timer=timers.get(node.key);if(timer){if(reduced()&&timer.id!==null){timer.remaining=Math.max(0,timer.due-now());win.clearTimeout(timer.id);timer.id=null;}else if(!reduced()&&timer.id===null)startTimer(node,timer);}}}
  }
  function onPreference(){if(mode==='system')setMotion('system');}
  const onPointerOverPause=event=>pauseFromEvent(event,'hover',true);
  const onPointerOutPause=event=>{const from=effectForTarget(event.target),to=effectForTarget(event.relatedTarget);if(from?.key!==to?.key&&from)pauseTimer(from,'hover',false);};
  const onFocusIn=event=>onFocus(event,true),onFocusOut=event=>{const from=effectForTarget(event.target),to=effectForTarget(event.relatedTarget);if(from?.key!==to?.key&&from){if(from.props.Effect==='ScrollReveal')onFocus(event,false);else pauseTimer(from,'focus',false);}};
  root.addEventListener('pointermove',onPointerMove);root.addEventListener('pointerout',onPointerOut,true);
  root.addEventListener('pointerover',onPointerOverPause);root.addEventListener('pointerout',onPointerOutPause);
  root.addEventListener('focusin',onFocusIn);root.addEventListener('focusout',onFocusOut);
  doc.addEventListener('visibilitychange',onVisibility);media?.addEventListener?.('change',onPreference);
  return {
    prepare,commit,setMotion,
    dispose(){if(disposed)return;disposed=true;root.removeEventListener('pointermove',onPointerMove);root.removeEventListener('pointerout',onPointerOut,true);root.removeEventListener('pointerover',onPointerOverPause);root.removeEventListener('pointerout',onPointerOutPause);root.removeEventListener('focusin',onFocusIn);root.removeEventListener('focusout',onFocusOut);doc.removeEventListener('visibilitychange',onVisibility);media?.removeEventListener?.('change',onPreference);intersection?.disconnect();for(const timer of timers.values())if(timer.id!==null)win.clearTimeout(timer.id);timers.clear();for(const [key,record] of records){cancelAnimations(record.wordAnimations,true);cancelAnimations(record.listAnimations?.values(),true);cancelAnimations(record.selectionAnimations?.values(),true);restoreSelection(key,currentEntries);}records.clear();revealed.clear();visible.clear();},
  };
}
