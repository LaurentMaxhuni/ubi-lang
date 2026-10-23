const modalKinds=new Set(['Dialog','AlertDialog','Drawer','CommandPalette']);
const roleFor={Dialog:'dialog',AlertDialog:'alertdialog',Drawer:'dialog',CommandPalette:'dialog',Popover:'region',Tooltip:'tooltip',Toast:'status',Select:'listbox',Combobox:'listbox',DropdownMenu:'menu',ContextMenu:'menu'};
const focusSelector='button:not(:disabled),input:not(:disabled),textarea:not(:disabled),select:not(:disabled),a[href],[tabindex]:not([tabindex="-1"])';
const clamp=(value,min,max)=>Math.min(max,Math.max(min,value));

function focusables(element) {
  return [...element.querySelectorAll(focusSelector)].filter(item=>!item.hidden&&item.getClientRects().length>0&&!item.closest('[inert]'));
}

function pointFor(anchor,overlay,placement,pointer,viewport) {
  const margin=8, box=overlay.getBoundingClientRect(), width=box.width||Math.min(320,viewport.width-16), height=box.height||80;
  if(pointer) return {left:clamp(pointer.x,margin,Math.max(margin,viewport.width-width-margin)),top:clamp(pointer.y,margin,Math.max(margin,viewport.height-height-margin))};
  if(!anchor||placement==='Center') return {left:(viewport.width-width)/2,top:(viewport.height-height)/2};
  const rect=anchor.getBoundingClientRect(), gap=8;
  let preferred=placement, left, top;
  const position=side=>{
    if(side==='Top') return {left:rect.left+(rect.width-width)/2,top:rect.top-height-gap};
    if(side==='Bottom') return {left:rect.left+(rect.width-width)/2,top:rect.bottom+gap};
    if(side==='Left') return {left:rect.left-width-gap,top:rect.top+(rect.height-height)/2};
    return {left:rect.right+gap,top:rect.top+(rect.height-height)/2};
  };
  ({left,top}=position(preferred));
  if(preferred==='Bottom'&&top+height>viewport.height-margin) preferred='Top';
  else if(preferred==='Top'&&top<margin) preferred='Bottom';
  else if(preferred==='Right'&&left+width>viewport.width-margin) preferred='Left';
  else if(preferred==='Left'&&left<margin) preferred='Right';
  ({left,top}=position(preferred));
  return {left:clamp(left,margin,Math.max(margin,viewport.width-width-margin)),top:clamp(top,margin,Math.max(margin,viewport.height-height-margin))};
}

/** Per-mount overlay stack, focus isolation and visible-time timers. */
export function createOverlays(root,options={}) {
  if(!root||root.nodeType!==1) throw new TypeError('Ubi overlays require a mount element');
  const doc=root.ownerDocument,win=doc.defaultView;
  const host=doc.createElement('div'); host.className='ubi-overlay-host'; host.dataset.ubiOverlayHost='';
  const records=new Map(),toastTimers=new Map(),requested=new Set(),hoverTimers=new Map(),pointerAnchors=new Map(),originalInert=new Map(),tooltipRelations=new Map();
  let currentView=null,currentEntries=new Map(),dispatch=()=>{},disposed=false,previousModal=[],outsideDown=null;
  const activeOptions=new Map();
  let typeaheadBuffer='',typeaheadScope='',typeaheadOverlayKey='',typeaheadTimer=null,typeaheadGeneration=0;
  const now=()=>win.performance.now();
  const getNode=key=>currentView?.nodes.find(node=>node.key===key);
  const isOpen=node=>node?.kind==='Overlay'&&node.props.Open;
  const openOverlays=()=>currentView?.nodes.filter(isOpen)??[];
  const getRecord=node=>{
    let record=records.get(node.key);
    if(!record){const backdrop=doc.createElement('div');backdrop.className='ubi-overlay-backdrop';backdrop.setAttribute('aria-hidden','true');backdrop.inert=true;record={backdrop,element:null,savedFocus:null,wasOpen:false};records.set(node.key,record);}
    return record;
  };
  const fire=(node,prop,context)=>{if(!disposed&&node?.props?.[prop]!==undefined) dispatch(node,prop,context);};
  function restoreTooltipRelations() {
    for(const [key,{element,value}] of tooltipRelations){if(element.isConnected){if(value===null)element.removeAttribute('aria-describedby');else element.setAttribute('aria-describedby',value);}}
    tooltipRelations.clear();
  }
  function addTooltipRelation(node,overlayElement) {
    const anchor=currentEntries.get(node.props.Anchor)?.element;
    if(!anchor||!overlayElement.id)return;
    if(!tooltipRelations.has(node.props.Anchor)) tooltipRelations.set(node.props.Anchor,{element:anchor,value:anchor.getAttribute('aria-describedby')});
    const {value}=tooltipRelations.get(node.props.Anchor),tokens=new Set((value??'').split(/\s+/u).filter(Boolean));tokens.add(overlayElement.id);
    anchor.setAttribute('aria-describedby',[...tokens].join(' '));
  }
  function placeOverlay(node,element,index) {
    const kind=node.props.OverlayKind,placement=node.props.Placement;
    element.classList.add('ubi-overlay'); element.dataset.ubiOverlayKind=kind;
    element.style.position='fixed';element.style.maxWidth='calc(100vw - 16px)';element.style.maxHeight='calc(100dvh - 16px)';
    if(kind==='Toast') {element.style.right='16px';element.style.left='auto';element.style.top=`${16+index*12}px`;element.style.bottom='auto';element.style.translate='none';return;}
    if(kind==='Drawer') {
      element.style.top='0';element.style.bottom='0';element.style.left=placement==='Left'?'0':'auto';element.style.right=placement==='Left'?'auto':'0';element.style.translate='none';return;
    }
    if(modalKinds.has(kind)||placement==='Center') {element.style.left='50%';element.style.top='50%';element.style.right='auto';element.style.bottom='auto';element.style.translate='-50% -50%';return;}
    const anchor=currentEntries.get(node.props.Anchor)?.element,pointer=pointerAnchors.get(node.key);
    const rect=pointFor(anchor,element,placement,pointer,{width:win.innerWidth,height:win.innerHeight});
    element.style.left=`${rect.left}px`;element.style.top=`${rect.top}px`;element.style.right='auto';element.style.bottom='auto';element.style.translate='none';
  }
  function restoreInert() {
    for(const [element,value] of originalInert){if(element.isConnected)element.inert=value;}
    originalInert.clear();
  }
  function isolate(modals) {
    restoreInert();
    if(!modals.length)return;
    const main=root.firstElementChild;
    if(main&&main!==host){originalInert.set(main,main.inert);main.inert=true;}
    const top=modals.at(-1).key;
    for(const node of openOverlays()) {
      const element=currentEntries.get(node.key)?.element;
      if(!element)continue;
      const inert=node.key!==top;
      if(element.inert!==inert){originalInert.set(element,element.inert);element.inert=inert;}
    }
  }
  function trapFocus(event) {
    const modal=previousModal.at(-1);if(!modal)return;
    const panel=currentEntries.get(modal.key)?.element;if(!panel)return;
    if(event.key==='Escape') {event.preventDefault();if(modal.props.OverlayKind!=='AlertDialog')fire(modal,'Dismiss');return;}
    if(event.key!=='Tab')return;
    const items=focusables(panel);if(!items.length){event.preventDefault();panel.focus({preventScroll:true});return;}
    const first=items[0],last=items.at(-1),active=doc.activeElement;
    if(event.shiftKey&&(active===first||!panel.contains(active))){event.preventDefault();last.focus();}
    else if(!event.shiftKey&&(active===last||!panel.contains(active))){event.preventDefault();first.focus();}
  }
  function enforceFocus(event) {
    const modal=previousModal.at(-1);if(!modal)return;
    const panel=currentEntries.get(modal.key)?.element;
    if(panel&&!panel.contains(event.target)) (focusables(panel)[0]??panel).focus({preventScroll:true});
  }
  function onBackdropDown(event) {if(event.target===event.currentTarget)outsideDown={key:event.currentTarget.dataset.key,pointerId:event.pointerId};else outsideDown=null;}
  function onBackdropUp(event) {
    const key=event.currentTarget.dataset.key;
    if(outsideDown?.key===key&&outsideDown.pointerId===event.pointerId&&event.target===event.currentTarget){const node=getNode(key);if(node?.props.OverlayKind!=='AlertDialog')fire(node,'Dismiss');}
    outsideDown=null;
  }
  function pauseTimer(key,reason,pause) {
    const timer=toastTimers.get(key);if(!timer)return;
    if(pause){timer.reasons.add(reason);if(!timer.paused){timer.remaining=Math.max(0,timer.due-now());win.clearTimeout(timer.id);timer.id=null;timer.paused=true;}}
    else {timer.reasons.delete(reason);if(timer.paused&&!timer.reasons.size){timer.paused=false;startTimer(key,timer);}}
  }
  function startTimer(key,timer) {
    timer.due=now()+timer.remaining;
    const generation=timer.generation;
    timer.id=win.setTimeout(()=>{const latest=toastTimers.get(key);if(latest!==timer||latest.generation!==generation||latest.paused)return;latest.id=null;latest.remaining=0;fire(getNode(key),'Dismiss');},Math.max(0,timer.remaining));
  }
  function syncTimers(nodes) {
    const live=new Set();
    for(const node of nodes) {
      const timeout=node.props.TimeoutMs??0;if(node.props.OverlayKind!=='Toast'||!node.props.Open||timeout<=0)continue;
      live.add(node.key);const existing=toastTimers.get(node.key),record=getRecord(node);
      if(existing&&record.wasOpen&&existing.timeout===timeout)continue;
      if(existing&&existing.id!==null)win.clearTimeout(existing.id);
      const timer={timeout,remaining:timeout,due:0,id:null,paused:false,reasons:new Set(),generation:(existing?.generation??0)+1};toastTimers.set(node.key,timer);
      if(doc.visibilityState==='hidden'){timer.reasons.add('hidden');timer.paused=true;}else startTimer(node.key,timer);
    }
    for(const [key,timer] of toastTimers)if(!live.has(key)){if(timer.id!==null)win.clearTimeout(timer.id);toastTimers.delete(key);}
  }
  function toastForTarget(target) {
    const element=target?.closest?.('[data-ubi-key]');if(!element)return null;
    let node=getNode(element.dataset.ubiKey);while(node){if(node.kind==='Overlay'&&node.props.OverlayKind==='Toast'&&node.props.Open)return node;node=currentView.nodes.find(parent=>parent.children.includes(node.key));}
    return null;
  }
  function onPointerOver(event) {const toast=toastForTarget(event.target);if(toast)pauseTimer(toast.key,'hover',true);}
  function onPointerOut(event) {const toast=toastForTarget(event.target);if(toast&&!toastForTarget(event.relatedTarget))pauseTimer(toast.key,'hover',false);}
  function onFocusIn(event) {const toast=toastForTarget(event.target);if(toast)pauseTimer(toast.key,'focus',true);}
  function onFocusOut(event) {const toast=toastForTarget(event.target);if(toast&&!toastForTarget(event.relatedTarget))pauseTimer(toast.key,'focus',false);}
  function onVisibility() {for(const key of toastTimers.keys())pauseTimer(key,'hidden',doc.visibilityState==='hidden');}
  function triggerFor(value) {const target=value?.target??value,keyed=target?.closest?.('[data-ubi-key]');return keyed&&root.contains(keyed)?keyed.dataset.ubiKey:null;}
  function tooltipForTarget(target) {
    const keyed=target?.closest?.('[data-ubi-key]');if(!keyed)return null;
    let node=getNode(keyed.dataset.ubiKey);
    while(node){if(node.kind==='Overlay'&&node.props.OverlayKind==='Tooltip'&&node.props.Open)return node;node=currentView.nodes.find(parent=>parent.children.includes(node.key));}
    return null;
  }
  function clearHover(key) {const timer=hoverTimers.get(key);if(timer!==undefined){win.clearTimeout(timer);hoverTimers.delete(key);}}
  function requestFor(anchorKey,kind,pointer) {
    const overlay=currentView?.nodes.find(node=>node.kind==='Overlay'&&node.props.Anchor===anchorKey&&node.props.OverlayKind===kind);
    if(!overlay||requested.has(overlay.key)||overlay.props.OpenRequest===undefined)return;
    if(pointer)pointerAnchors.set(overlay.key,pointer);else pointerAnchors.delete(overlay.key);
    requested.add(overlay.key);fire(overlay,'OpenRequest',{type:'anchor-open',anchorKey});
  }
  function onFocusTrigger(event) {
    const key=triggerFor(event);if(!key)return;
    const tooltip=currentView?.nodes.find(node=>node.kind==='Overlay'&&node.props.OverlayKind==='Tooltip'&&node.props.Anchor===key&&!node.props.Open);
    if(tooltip)requestFor(key,'Tooltip');
  }
  function onBlurTrigger(event) {
    const key=triggerFor(event);if(!key||triggerFor(event.relatedTarget)===key)return;
    const tooltip=openOverlays().find(node=>node.props.OverlayKind==='Tooltip'&&node.props.Anchor===key);
    if(tooltip)fire(tooltip,'Dismiss');
  }
  function onHoverTrigger(event) {
    if(event.type==='pointerout') {
      const content=tooltipForTarget(event.target);
      if(content) {
        const anchor=currentEntries.get(content.props.Anchor)?.element,element=currentEntries.get(content.key)?.element;
        if(anchor?.contains(event.relatedTarget)||element?.contains(event.relatedTarget))return;
        clearHover(content.key);fire(content,'Dismiss');return;
      }
    }
    const key=triggerFor(event);if(!key)return;
    if(event.type==='pointerover') {
      if(event.pointerType==='touch'||triggerFor(event.relatedTarget)===key)return;
      const tooltip=currentView?.nodes.find(node=>node.kind==='Overlay'&&node.props.OverlayKind==='Tooltip'&&node.props.Anchor===key&&!node.props.Open);
      if(tooltip&&!hoverTimers.has(tooltip.key))hoverTimers.set(tooltip.key,win.setTimeout(()=>{hoverTimers.delete(tooltip.key);requestFor(key,'Tooltip');},350));
    } else {
      if(triggerFor(event.relatedTarget)===key)return;
      for(const node of currentView?.nodes??[])if(node.props.OverlayKind==='Tooltip'&&node.props.Anchor===key)clearHover(node.key);
      const tooltip=openOverlays().find(node=>node.props.OverlayKind==='Tooltip'&&node.props.Anchor===key);
      if(tooltip&&!currentEntries.get(tooltip.key)?.element.contains(event.relatedTarget))fire(tooltip,'Dismiss');
    }
  }
  function onContextMenu(event) {
    const key=triggerFor(event);if(!key)return;
    const overlay=currentView?.nodes.find(node=>node.kind==='Overlay'&&node.props.OverlayKind==='ContextMenu'&&node.props.Anchor===key&&!node.props.Open);
    if(!overlay)return;
    event.preventDefault();requestFor(key,'ContextMenu',{x:event.clientX,y:event.clientY});
  }
  function onContextShortcut(event) {
    if(event.key!=='F10'||!event.shiftKey||event.altKey||event.ctrlKey||event.metaKey)return;
    const key=triggerFor(event);if(!key)return;
    const overlay=currentView?.nodes.find(node=>node.kind==='Overlay'&&node.props.OverlayKind==='ContextMenu'&&node.props.Anchor===key&&!node.props.Open);
    if(!overlay)return;
    event.preventDefault();requestFor(key,'ContextMenu');
  }
  function onResize() {for(const node of openOverlays()){const element=currentEntries.get(node.key)?.element;if(element)placeOverlay(node,element,0);}}
  function onScroll() {for(const node of openOverlays())if(node.props.Anchor){const element=currentEntries.get(node.key)?.element;if(element)placeOverlay(node,element,0);}}
  function resetTypeahead() {
    if(typeaheadTimer!==null)win.clearTimeout(typeaheadTimer);
    typeaheadTimer=null;typeaheadBuffer='';typeaheadScope='';typeaheadOverlayKey='';typeaheadGeneration++;
  }
  function menuOverlayFor(node) {
    const parents=new Map();for(const item of currentView?.nodes??[])for(const key of item.children)parents.set(key,item.key);
    let key=node.key;
    while(key){const parentKey=parents.get(key);if(!parentKey)return null;const parent=currentView.nodes.find(item=>item.key===parentKey);if(parent?.kind==='Overlay')return parent;key=parentKey;}
    return null;
  }
  function comboboxInputForOption(node) {
    const overlay=menuOverlayFor(node);
    return overlay?.props.OverlayKind==='Combobox'?currentEntries.get(overlay.props.Anchor)?.control??null:null;
  }
  function handleComboboxInputKeydown(event,node,onSelect=()=>{}) {
    if(node?.kind!=='TextInput'||event.altKey||event.ctrlKey||event.metaKey||event.isComposing||event.keyCode===229)return false;
    const overlay=openOverlays().find(item=>item.props.OverlayKind==='Combobox'&&item.props.Anchor===node.key);
    if(!overlay)return false;
    const peers=overlayItems(overlay,'OptionItem').filter(item=>!item.props.Disabled&&!item.props.Busy);
    if(!peers.length)return false;
    const active=activeOptions.get(overlay.key),index=peers.findIndex(item=>item.key===active);
    if(['ArrowDown','ArrowUp','Home','End'].includes(event.key)) {
      const direction=event.key==='ArrowUp'?-1:1;
      const target=event.key==='Home'?peers[0]:event.key==='End'?peers.at(-1):peers[index<0?(direction>0?0:peers.length-1):(index+direction+peers.length)%peers.length];
      event.preventDefault();setActiveOption(overlay,target);return true;
    }
    if(event.key==='Enter'&&!event.repeat) {
      const target=peers[index];if(!target)return false;
      event.preventDefault();onSelect(target);return true;
    }
    return false;
  }
  function overlayItems(overlay,kind) {
    const byKey=new Map(currentView?.nodes.map(item=>[item.key,item])??[]),keys=new Set(),stack=[overlay.key];
    while(stack.length){const key=stack.pop(),item=byKey.get(key);if(!item||keys.has(key))continue;keys.add(key);stack.push(...item.children);}
    return currentView.nodes.filter(item=>keys.has(item.key)&&item.kind===kind);
  }
  function setActiveOption(overlay,item) {
    const anchor=currentEntries.get(overlay.props.Anchor)?.control;
    const option=currentEntries.get(item?.key)?.element;
    if(!anchor||!option?.id){anchor?.removeAttribute('aria-activedescendant');activeOptions.delete(overlay.key);return;}
    activeOptions.set(overlay.key,item.key);anchor.setAttribute('aria-activedescendant',option.id);
  }
  function handleMenuKeydown(event,node) {
    if(!['MenuItem','OptionItem'].includes(node?.kind)||event.altKey||event.ctrlKey||event.metaKey||event.isComposing)return false;
    const overlay=menuOverlayFor(node);if(!overlay?.props.Open)return false;
    const descendants=new Set();
    const collect=key=>{const item=currentView.nodes.find(candidate=>candidate.key===key);if(!item)return;descendants.add(key);item.children.forEach(collect);};collect(overlay.key);
    const peers=currentView.nodes.filter(item=>descendants.has(item.key)&&item.kind===node.kind&&!item.props.Disabled&&!item.props.Busy);
    if(!peers.length)return false;
    let target;
    if(['ArrowLeft','ArrowUp','ArrowRight','ArrowDown','Home','End'].includes(event.key)) {
      const index=peers.findIndex(item=>item.key===node.key),direction=['ArrowLeft','ArrowUp'].includes(event.key)?-1:1;
      target=peers[event.key==='Home'?0:event.key==='End'?peers.length-1:(index+direction+peers.length)%peers.length];
      resetTypeahead();
    } else {
      if(Array.from(event.key??'').length!==1||/\s/u.test(event.key)||/^\p{C}$/u.test(event.key))return false;
      const scope=`${overlay.key}:${node.kind}`,character=event.key;
      if(typeaheadScope!==scope){resetTypeahead();typeaheadScope=scope;typeaheadOverlayKey=overlay.key;}
      const fold=value=>value.normalize('NFD').replace(/\p{M}/gu,'').toLowerCase();
      const previous=Array.from(typeaheadBuffer),last=fold(character);
      const repeated=previous.length>0&&previous.every(part=>fold(part)===last);
      const next=repeated?character:[...previous,character].slice(-64).join('');
      typeaheadBuffer=next;
      const generation=++typeaheadGeneration;
      if(typeaheadTimer!==null)win.clearTimeout(typeaheadTimer);
      typeaheadTimer=win.setTimeout(()=>{if(generation===typeaheadGeneration)resetTypeahead();},500);
      const query=fold(next),index=peers.findIndex(item=>item.key===node.key);
      target=Array.from({length:peers.length},(_,offset)=>peers[(index+offset+1)%peers.length]).find(item=>fold(item.props.Label).startsWith(query));
      if(!target)return false;
    }
    const control=currentEntries.get(target.key)?.control??currentEntries.get(target.key)?.element;
    if(!control)return false;
    event.preventDefault();control.focus();
    return true;
  }
  function commit(view,entries,onDispatch=dispatch,interactionFocus=doc.activeElement) {
    if(disposed)throw new TypeError('Ubi overlays are disposed');
    const previousTop=previousModal.at(-1),previousFocus=interactionFocus,previousEntries=currentEntries,previousView=currentView;
    const closing=[],justOpened=[];
    const previousParents=new Map();for(const node of previousView?.nodes??[])for(const child of node.children)previousParents.set(child,node.key);
    const focusedKey=previousFocus?.closest?.('[data-ubi-key]')?.dataset?.ubiKey;
    const focusWithin=key=>{for(let current=focusedKey;current;current=previousParents.get(current))if(current===key)return true;return false;};
    for(const [key,record] of records) {
      if(!record.wasOpen)continue;
      const next=view.nodes.find(node=>node.key===key&&node.kind==='Overlay');
      if(next?.props.Open)continue;
      const prior=previousView?.nodes.find(node=>node.key===key&&node.kind==='Overlay');
      const inside=Boolean(record.element?.contains?.(previousFocus))||focusWithin(key)||Boolean(prior?.props.OverlayKind==='Combobox'&&previousFocus===record.savedFocus);
      closing.push({key,record,prior,inside});
    }
    currentView=view;currentEntries=entries;dispatch=onDispatch;
    const nodes=view.nodes.filter(node=>node.kind==='Overlay'),open=nodes.filter(node=>node.props.Open);
    if(typeaheadOverlayKey&&!open.some(node=>node.key===typeaheadOverlayKey))resetTypeahead();
    restoreTooltipRelations();
    for(const node of nodes) {
      const element=entries.get(node.key)?.element;if(!element)continue;
      const record=getRecord(node),opened=node.props.Open,wasOpen=record.wasOpen;record.element=element;
      element.classList.add('ubi-overlay');
      element.hidden=!opened;element.inert=!opened;
      if(!opened){element.removeAttribute('aria-modal');element.removeAttribute('data-ubi-overlay-kind');element.style.position='';element.style.left='';element.style.right='';element.style.top='';element.style.bottom='';element.style.translate='';record.backdrop.remove();}
      else {
        element.dataset.ubiOverlayKind=node.props.OverlayKind;
        element.setAttribute('role',node.props.Role??roleFor[node.props.OverlayKind]);
        if(node.props.LabelledBy)element.setAttribute('aria-labelledby',entries.get(node.props.LabelledBy)?.element.id??'');
        else if(node.props.Label)element.setAttribute('aria-label',node.props.Label);
        if(modalKinds.has(node.props.OverlayKind))element.setAttribute('aria-modal','true');
        if(node.props.OverlayKind==='Tooltip')addTooltipRelation(node,element);
        host.append(element);
      }
      if(wasOpen!==opened)requested.delete(node.key);
      if(wasOpen&&!opened)pointerAnchors.delete(node.key);
      if(opened&&!wasOpen) {
        record.anchorKey=node.props.Anchor??null;
        const anchor=node.props.Anchor?entries.get(node.props.Anchor):null;
        record.savedFocus=anchor?.control??anchor?.element??previousFocus;
        justOpened.push(node);
      }
      record.wasOpen=opened;
    }
    const openedNodes=openOverlays();
    const modals=openedNodes.filter(node=>modalKinds.has(node.props.OverlayKind));
    const modalKeys=new Set(modals.map(node=>node.key));
    let modalIndex=0,toastIndex=0;
    for(const node of openedNodes) {
      const element=entries.get(node.key)?.element;if(!element)continue;
      if(modalKeys.has(node.key)) {
        const record=getRecord(node);record.backdrop.dataset.key=node.key;
        record.backdrop.onpointerdown=onBackdropDown;record.backdrop.onpointerup=onBackdropUp;
        if(record.backdrop.parentElement!==host)host.insertBefore(record.backdrop,element);
        record.backdrop.style.zIndex=String(10+modalIndex*2);element.style.zIndex=String(11+modalIndex*2);
        if(!previousModal.some(item=>item.key===node.key))record.savedFocus=previousFocus;
        modalIndex++;
      } else if(node.props.OverlayKind==='Toast') element.style.zIndex=String(100+toastIndex++);
      else element.style.zIndex=String(50+openedNodes.indexOf(node));
      placeOverlay(node,element,node.props.OverlayKind==='Toast'?toastIndex-1:0);
    }
    for(const node of openedNodes)if(modalKeys.has(node.key))host.append(entries.get(node.key).element);
    if(root.firstElementChild)root.append(host);
    const nextTop=modals.at(-1);
    previousModal=modals;
    isolate(modals);
    syncTimers(nodes);
    for(const node of open)if(node.props.OverlayKind==='Combobox') {
      const peers=overlayItems(node,'OptionItem').filter(item=>!item.props.Disabled&&!item.props.Busy);
      let target=peers.find(item=>item.key===activeOptions.get(node.key));
      if(!target)target=peers.find(item=>item.props.Selected===true||item.props.Checked===true)??peers[0];
      if(target)setActiveOption(node,target);
      else {entries.get(node.props.Anchor)?.control?.removeAttribute('aria-activedescendant');activeOptions.delete(node.key);}
    }
    for(const {key,record,prior} of closing) {
      if(prior?.props.OverlayKind==='Combobox')previousEntries.get(prior.props.Anchor)?.control?.removeAttribute('aria-activedescendant');
      activeOptions.delete(key);
    }
    for(const node of nodes)if(node.props.Open)requested.delete(node.key);
    if(nextTop&&previousTop?.key!==nextTop.key) {
      const panel=entries.get(nextTop.key)?.element;
      if(panel){panel.tabIndex=panel.tabIndex<0?panel.tabIndex:-1;(focusables(panel)[0]??panel).focus({preventScroll:true});}
    } else if(!nextTop&&previousTop) {
      const focus=records.get(previousTop.key)?.savedFocus;
      if(focus?.isConnected&&!focus.closest('[hidden],[inert]'))focus.focus({preventScroll:true});
    } else if(!nextTop) {
      const restore=closing.find(item=>item.inside&&['Popover','Select','DropdownMenu','ContextMenu','Combobox'].includes(item.prior?.props.OverlayKind));
      if(restore) {
        const target=restore.record.anchorKey?entries.get(restore.record.anchorKey)?.control??entries.get(restore.record.anchorKey)?.element:restore.record.savedFocus;
        const fallback=target?.isConnected&&!target.closest?.('[hidden],[inert]')?target:restore.record.savedFocus;
        if(fallback?.isConnected&&!fallback.closest?.('[hidden],[inert]'))fallback.focus({preventScroll:true});
      }
    }
    const openedNonModals=justOpened.filter(node=>!modalKeys.has(node.key)&&['Select','DropdownMenu','ContextMenu'].includes(node.props.OverlayKind));
    if(!nextTop&&openedNonModals.length) {
      const node=openedNonModals.at(-1),record=records.get(node.key),items=overlayItems(node,node.props.OverlayKind==='Select'?'OptionItem':'MenuItem').filter(item=>!item.props.Disabled&&!item.props.Busy);
      const target=node.props.OverlayKind==='Select'?items.find(item=>item.props.Selected===true||item.props.Checked===true)??items[0]:items[0];
      const control=target&&(entries.get(target.key)?.control??entries.get(target.key)?.element);
      if(control)control.focus({preventScroll:true});
    }
      for(const [key,record] of records)if(!nodes.some(node=>node.key===key)){record.element?.remove();record.backdrop.remove();records.delete(key);activeOptions.delete(key);pointerAnchors.delete(key);}
    if(nextTop){doc.addEventListener('keydown',trapFocus,true);doc.addEventListener('focusin',enforceFocus,true);}
    else {doc.removeEventListener('keydown',trapFocus,true);doc.removeEventListener('focusin',enforceFocus,true);}
  }
  function pointerDown(event) {
    const key=triggerFor(event);if(!key||event.button!==2)return;
    const node=currentView?.nodes.find(item=>item.kind==='Overlay'&&item.props.OverlayKind==='ContextMenu'&&item.props.Anchor===key&&item.props.Open);
    if(node)pointerAnchors.set(node.key,{x:event.clientX,y:event.clientY});
  }
  function onEscape(event){if(event.key==='Escape'&&previousModal.length===0){const node=openOverlays().at(-1);if(node&&node.props.OverlayKind!=='AlertDialog'){event.preventDefault();fire(node,'Dismiss');}}}
  root.addEventListener('pointerover',onHoverTrigger,true);root.addEventListener('pointerout',onHoverTrigger,true);
  root.addEventListener('focusin',onFocusTrigger,true);root.addEventListener('focusout',onBlurTrigger,true);
  root.addEventListener('contextmenu',onContextMenu,true);root.addEventListener('keydown',onContextShortcut,true);root.addEventListener('pointerdown',pointerDown,true);
  root.addEventListener('pointerover',onPointerOver);root.addEventListener('pointerout',onPointerOut);
  root.addEventListener('focusin',onFocusIn);root.addEventListener('focusout',onFocusOut);
  doc.addEventListener('visibilitychange',onVisibility);doc.addEventListener('keydown',onEscape,true);doc.addEventListener('scroll',onScroll,true);win.addEventListener('resize',onResize);
  return {
    commit,
    handleMenuKeydown,
    handleComboboxInputKeydown,
    comboboxInputForOption,
    dispose(){
      if(disposed)return;disposed=true;
      resetTypeahead();
      root.removeEventListener('pointerover',onHoverTrigger,true);root.removeEventListener('pointerout',onHoverTrigger,true);
      root.removeEventListener('focusin',onFocusTrigger,true);root.removeEventListener('focusout',onBlurTrigger,true);
      root.removeEventListener('contextmenu',onContextMenu,true);root.removeEventListener('keydown',onContextShortcut,true);root.removeEventListener('pointerdown',pointerDown,true);
      root.removeEventListener('pointerover',onPointerOver);root.removeEventListener('pointerout',onPointerOut);
      root.removeEventListener('focusin',onFocusIn);root.removeEventListener('focusout',onFocusOut);
      doc.removeEventListener('visibilitychange',onVisibility);doc.removeEventListener('keydown',onEscape,true);doc.removeEventListener('keydown',trapFocus,true);doc.removeEventListener('focusin',enforceFocus,true);doc.removeEventListener('scroll',onScroll,true);win.removeEventListener('resize',onResize);
      for(const timer of hoverTimers.values())win.clearTimeout(timer);hoverTimers.clear();
      for(const timer of toastTimers.values())if(timer.id!==null)win.clearTimeout(timer.id);toastTimers.clear();
      restoreTooltipRelations();restoreInert();pointerAnchors.clear();
      for(const record of records.values())record.backdrop.remove();records.clear();
      host.remove();previousModal=[];currentView=null;currentEntries.clear();
    },
  };
}
