import { encode } from './codec.mjs';
import { validateView, containers, editable, interactive } from './validate.mjs';
import { createPresentation } from './presentation.mjs';
import { createOverlays } from './overlays.mjs';
import { createEffects } from './effects.mjs';
export { validateView } from './validate.mjs';

const owners = new WeakMap();
let nextMount = 0;
const clone = value => structuredClone(value);
// Public Ubi wrappers accept primitives directly and aggregates as JSON text.
export function argument(value) {
  const encoded = encode(value);
  return value !== null && typeof value === 'object' ? encoded : value;
}
const enabled = node => !node.props.Disabled && !node.props.Busy && (node.kind!=='TableHeader'||node.props.Change!==undefined);
const isGroupItem = node => ['Radio','Tab','ToggleItem'].includes(node.kind) && node.props.Group;
const isActionable = node => interactive.has(node.kind) || (node.kind==='TableHeader'&&node.props.Change!==undefined);
const attr = (element, name, value) => value===undefined || value===null || value==='' ? element.removeAttribute(name) : element.setAttribute(name,String(value));
function place(parent, children) {
  children.forEach((child,i)=>{ if(parent.childNodes[i]!==child) parent.insertBefore(child,parent.childNodes[i]??null); });
  while (parent.childNodes.length>children.length) parent.lastChild.remove();
}
function snapshot(root) {
  const document=root.ownerDocument, focus=document.activeElement;
  const entries=[root,...root.querySelectorAll('*')].map(element=>({element,children:[...element.childNodes],attributes:[...element.attributes].map(a=>[a.name,a.value]),value:element.value,checked:element.checked,scroll:[element.scrollLeft,element.scrollTop]}));
  const texts=entries.flatMap(({children})=>children.filter(child=>child.nodeType===3).map(child=>[child,child.nodeValue]));
  const selection=focus && typeof focus.selectionStart==='number' ? [focus.selectionStart,focus.selectionEnd,focus.selectionDirection] : null;
  return () => {
    for(const {element,children,attributes,value,checked,scroll} of entries) {
      element.getAttributeNames().forEach(name=>element.removeAttribute(name));
      attributes.forEach(([name,value])=>element.setAttribute(name,value)); element.replaceChildren(...children);
      if(value!==undefined) element.value=value;
      if(checked!==undefined) element.checked=checked;
      [element.scrollLeft,element.scrollTop]=scroll;
    }
    texts.forEach(([text,value])=>{text.nodeValue=value;});
    if(root.contains(focus)) { focus.focus({preventScroll:true}); if(selection) focus.setSelectionRange(...selection); }
  };
}
function retainInteraction(root) {
  const focus=root.ownerDocument.activeElement;
  const selection=focus && typeof focus.selectionStart==='number' ? [focus.selectionStart,focus.selectionEnd,focus.selectionDirection] : null;
  const scrolled=[root,...root.querySelectorAll('*')].filter(element=>element.scrollTop||element.scrollLeft).map(element=>[element,element.scrollLeft,element.scrollTop]);
  return () => {
    if(root.contains(focus)) {
      if(root.ownerDocument.activeElement!==focus) focus.focus({preventScroll:true});
      if(selection) focus.setSelectionRange(...selection);
    }
    for(const [element,left,top] of scrolled) if(root.contains(element)||element===root) {element.scrollLeft=left;element.scrollTop=top;}
  };
}
function tagFor(node) {
  if(editable.has(node.kind) || ['Checkbox','Switch','Radio','Slider'].includes(node.kind)) return 'label';
  if(['Button','IconButton','Toggle','ToggleItem','Tab','Link','MenuItem','OptionItem'].includes(node.kind) || (node.kind==='TableHeader'&&node.props.Change!==undefined)) return 'button';
  if(node.kind==='TableHeader'||node.kind==='TableCell'||node.kind==='Table'||node.kind==='TableRow'||node.kind==='EffectSurface'||node.kind==='Overlay'||['Accordion','Collapsible','Pagination'].includes(node.kind)) return 'div';
  return {Heading:`h${node.props.Level}`,Text:'p',Kbd:'kbd',Label:'span',Badge:'span',Avatar:'span',ProgressBar:'div',Separator:'hr'}[node.kind]??'div';
}
function create(document,node) {
  const element=document.createElement(tagFor(node)); let control=element, caption;
  if(element.localName==='label') {
    control=document.createElement(node.kind==='TextArea'?'textarea':'input');
    if(control.localName==='input') control.type={PasswordInput:'password',NumberInput:'number',Checkbox:'checkbox',Switch:'checkbox',Radio:'radio',Slider:'range'}[node.kind]??'text';
    caption=document.createElement('span'); caption.className='ubi-caption';
    if(editable.has(node.kind) || node.kind==='Slider') element.append(caption,control); else element.append(control,caption);
  }
  if(element.localName==='button') element.type='button';
  const entry={element,control,caption,kind:node.kind,effect:node.props.Effect};
  if(node.kind==='ProgressBar') {
    entry.progressFill=document.createElement('span');entry.progressFill.className='ubi-progress-fill';entry.progressFill.setAttribute('aria-hidden','true');element.append(entry.progressFill);
  }
  if(node.props.Effect==='NumberTicker'||node.props.Effect==='TextReveal') {
    entry.semantic=document.createElement('span');entry.semantic.className='ubi-sr-only';
    entry.visual=document.createElement('span');entry.visual.setAttribute('aria-hidden','true');
    if(node.props.Effect==='NumberTicker') {
      entry.semantic.classList.add('ubi-number-semantic');
      entry.numberVisual=entry.visual;entry.numberVisual.className='ubi-number-visual';
    } else {
      entry.semantic.classList.add('ubi-text-reveal-semantic');entry.textVisual=entry.visual;entry.textVisual.className='ubi-text-reveal-visual';
    }
    element.append(entry.semantic,entry.visual);
  }
  return entry;
}
function styles(element,node) {
  const p=node.props,s=node.style,style=element.style;
  style.cssText='';
  style.gap=`${s.gap}px`;
  if(node.kind==='Window'||node.kind==='Card') style.padding=`${s.padding}px`;
  if(node.kind==='Heading') style.fontSize=`${s.fontSize}px`;
  // Explicit Ubi values outrank component and compact-preview CSS defaults.
  if(p.Padding!==undefined) style.setProperty('padding',`${s.padding}px`,'important');
  if(p.Gap!==undefined) style.setProperty('gap',`${s.gap}px`,'important');
  if(p.FontSize!==undefined) style.setProperty('font-size',`${s.fontSize}px`,'important');
  if(s.width!==undefined) style.width=s.width;
  if(s.height!==undefined) style.height=s.height;
  if(p.MinWidth!==undefined) style.minWidth=`min(100%, ${p.MinWidth}px)`;
  if(p.MinHeight!==undefined) style.minHeight=`${p.MinHeight}px`;
  if(p.MaxWidth!==undefined) style.maxWidth=`min(100%, ${p.MaxWidth}px)`;
  if(p.MaxHeight!==undefined) style.maxHeight=`${p.MaxHeight}px`;
  if(p.Grow!==undefined) style.flexGrow=String(p.Grow);
  if(node.kind==='Grid') style.gridTemplateColumns=`repeat(${s.columns}, minmax(0, 1fr))`;
  if(p.Center) { style.alignItems='center'; style.justifyContent='center'; style.textAlign='center'; }
  if(p.Clip) style.overflow='hidden';
  if(p.Absolute) { style.position='absolute'; style.left=`${p.Absolute[0]}px`; style.top=`${p.Absolute[1]}px`; }
}
/** Mount compiled Ubi init/update/view exports. Import renderer.css once per page. */
export function mount(root, app, options = {}) {
  if(!root || root.nodeType!==1) throw new TypeError('Ubi UI mount requires an element');
  for(const name of ['init','view','update']) if(typeof app?.[name]!=='function') throw new TypeError(`Missing Ubi app ${name}`);
  if(options.theme!==undefined && !['light','dark','system'].includes(options.theme)) throw new TypeError('Unsupported Ubi theme');
  if(options.motion!==undefined&&!['system','full','reduced'].includes(options.motion)) throw new TypeError('Unsupported Ubi motion mode');
  owners.get(root)?.dispose();
  const id=`ubi-web-${++nextMount}`,document=root.ownerDocument;
  const media=document.defaultView.matchMedia?.('(prefers-color-scheme: dark)');
  const presentation=createPresentation(root,options),overlays=createOverlays(root),effects=createEffects(root,options);
  let theme=options.theme, motionMode=options.motion??'system', model, view, rawView, elements=new Map(), bindings=new Map(), revision=0, closed=false, processing=false, rendering=false, broken=false;
  function report(error) {
    try { options.onError?.(error instanceof Error?error:new Error(String(error))); }
    catch(fault) { console.error('Ubi UI error handler failed',fault); }
  }
  function applyTheme(next=view, nextElements=elements) {
    for(const node of next?.nodes??[]) {
      const mode=node.key===next.root ? theme??node.props.Theme?.toLowerCase()??'system' : node.props.Theme?.toLowerCase();
      attr(nextElements.get(node.key).element,'data-ubi-theme',mode==='system'?media?.matches?'dark':'light':mode);
    }
  }
  function patch(next) {
    root.setAttribute('data-ubi-mount',id);
    attr(root,'data-ubi-preview',options.preview?'true':null);
    const nextElements=new Map(), nextBindings=new Map();
    for(const [i,node] of next.nodes.entries()) {
      const old=elements.get(node.key);
      const entry=old?.kind===node.kind && old.effect===node.props.Effect && old.element.localName===tagFor(node)?old:create(document,node);
      const {element,control,caption}=entry,p=node.props;
      if(!element.id) element.id=`${id}-${revision+1}-${i}`;
      element.className=`ubi-node ubi-${node.kind.toLowerCase()}`;
      element.setAttribute('data-ubi-key',node.key);
      attr(element,'data-ubi-effect',p.Effect);
      element.hidden=Boolean((node.kind==='Overlay'&&!p.Open)||(options.preview && next.nodes.find(item=>item.key===next.root)?.children.includes(node.key) && ['title','description'].includes(node.key)));
      if(node.kind==='Overlay')element.inert=!p.Open;
      if(node.props.Role==='tablist')element.classList.add('ubi-tablist');
      attr(element,'data-tone',p.Tone.toLowerCase()); attr(element,'data-variant',p.Variant.toLowerCase());
      attr(element,'data-checked',p.Checked?'true':null);
      attr(element,'data-disabled',enabled(node)?null:'true');
      styles(element,node);
      if(['Button','IconButton','Link','Switch','Tab','ToggleItem'].includes(node.kind)) {
        const motion=p.Motion??{durationMs:['Switch','Tab','ToggleItem'].includes(node.kind)?220:140,easing:['Switch','Tab','ToggleItem'].includes(node.kind)?'Spring':'EaseOut'};
        const easing={Linear:'linear',EaseOut:'cubic-bezier(.16,1,.3,1)',EaseInOut:'ease-in-out',Spring:'cubic-bezier(.2,1.3,.5,1)'}[motion.easing]??'cubic-bezier(.16,1,.3,1)';
        element.style.setProperty('--ubi-feedback-duration',`${motion.durationMs}ms`);
        element.style.setProperty('--ubi-feedback-easing',easing);
      }
      const role=p.Role??({Window:'region',Alert:'alert',Avatar:'img',Link:'link',Tab:'tab',Switch:'switch',ProgressBar:'progressbar',Overlay:({Dialog:'dialog',AlertDialog:'alertdialog',Drawer:'dialog',CommandPalette:'dialog',Popover:'region',Tooltip:'tooltip',Toast:'status',Select:'listbox',Combobox:'listbox',DropdownMenu:'menu',ContextMenu:'menu'})[p.OverlayKind],MenuItem:'menuitem',OptionItem:'option',Table:'table',TableRow:'row',TableHeader:'columnheader',TableCell:'cell'}[node.kind]);
      attr(control,'role',role);
      attr(control,'aria-label',p.Label || (node.kind==='Avatar'?p.Content:null));
      attr(control,'aria-busy',p.Busy?'true':null);
      attr(control,'aria-expanded',node.kind==='Accordion'||node.kind==='Collapsible'?p.Disclosure:p.Expanded);
      attr(control,'aria-disabled',!enabled(node)?'true':null);
      attr(control,'aria-readonly',p.ReadOnly?'true':null);
      attr(control,'aria-selected',node.kind==='OptionItem'?p.Selected:node.kind==='Tab'?p.Checked:undefined);
      attr(control,'aria-pressed',['Toggle','ToggleItem'].includes(node.kind)?p.Checked:node.kind==='Button'&&p.Pressed?true:undefined);
      if(node.kind==='TextInput') {attr(control,'aria-controls',null);attr(control,'aria-activedescendant',null);attr(control,'aria-haspopup',null);attr(control,'aria-autocomplete',null);}
      if('disabled' in control && isActionable(node)) control.disabled=!enabled(node);
      if(editable.has(node.kind)) {
        attr(control,'placeholder',p.Content);
        control.readOnly=p.ReadOnly; control.maxLength=16384;
        if(control.value!==p.Value) {
          const selection=document.activeElement===control && typeof control.selectionStart==='number'?[control.selectionStart,control.selectionEnd,control.selectionDirection]:null;
          control.value=p.Value;
          if(selection) control.setSelectionRange(...selection);
        }
        if(node.kind==='NumberInput') {control.min=String(p.Min); control.max=String(p.Max); control.step=String(p.Step);}
      }
      if(['Checkbox','Switch','Radio'].includes(node.kind)) control.checked=p.Checked;
      if(node.kind==='Radio') {control.name=`${id}-${p.Group||node.key}`; control.value=p.Option;}
      if(node.kind==='Slider') {control.min='0';control.max='100';control.step='1';control.value=String(p.Progress);}
      if(node.kind==='ProgressBar') {
        attr(element,'aria-valuemin','0');attr(element,'aria-valuemax','100');attr(element,'aria-valuenow',String(p.Progress));
        attr(element,'aria-label',p.Label);
        entry.progressFill.style.setProperty('--ubi-progress-visual',`${p.Progress}%`);
      }
      if(entry.semantic) {
        const text=entry.effect==='NumberTicker'?String(p.Number):String(p.Label??p.Content??'');
        if(entry.semantic.textContent!==text)entry.semantic.textContent=text;
        if(entry.numberVisual && entry.numberVisual.textContent!==String(p.Number))entry.numberVisual.textContent=String(p.Number);
        if(entry.textVisual && !p.Content && entry.textVisual.textContent)entry.textVisual.textContent='';
        if(entry.semantic.parentElement===element)element.classList.toggle('ubi-text-reveal',entry.effect==='TextReveal');
        if(entry.effect==='NumberTicker') {attr(element,'role','status');element.setAttribute('aria-live','polite');element.setAttribute('aria-atomic','true');attr(element,'aria-label',p.Label);}
      }
      if(caption && caption.textContent!==(p.Label||p.Content)) caption.textContent=p.Label||p.Content;
      if(!caption && !entry.semantic && !node.children.length && node.kind!=='ProgressBar' && element.textContent!==p.Content) element.textContent=p.Content;
      if(node.kind==='Separator') attr(element,'aria-orientation','horizontal');
      if(isActionable(node)) nextBindings.set(control,{node,revision:revision+1,control});
      nextElements.set(node.key,entry);
    }
    for(const node of next.nodes) {
      const element=nextElements.get(node.key).element,control=nextElements.get(node.key).control,p=node.props;
      attr(control,'aria-labelledby',p.LabelledBy?nextElements.get(p.LabelledBy)?.element.id:undefined);
      attr(control,'aria-describedby',p.DescribedBy?nextElements.get(p.DescribedBy)?.element.id:undefined);
    }
    for(const node of next.nodes) if(node.kind==='Overlay'&&node.props.OverlayKind==='Combobox') {
      const anchor=nextElements.get(node.props.Anchor)?.control,overlay=nextElements.get(node.key)?.element;
      if(!anchor)continue;
      attr(anchor,'role','combobox');attr(anchor,'aria-haspopup','listbox');attr(anchor,'aria-autocomplete','list');
      attr(anchor,'aria-controls',overlay?.id);attr(anchor,'aria-expanded',node.props.Open?'true':'false');
    }
    for(const node of next.nodes) if(containers.has(node.kind)) {
      const entry=nextElements.get(node.key);
      // Leaf content and effect spans belong to the renderer, not the source child list.
      if(entry.semantic) place(entry.element,[entry.semantic,entry.visual]);
      else if(node.children.length || !node.props.Content) place(entry.element,node.children.map(key=>nextElements.get(key).element));
    }
    // Roving focus is scoped to this mount and source group identity.
    for(const node of next.nodes) {
      if(!isGroupItem(node)) continue;
      const peers=next.nodes.filter(peer=>peer.kind===node.kind && peer.props.Group===node.props.Group && enabled(peer));
      const target=peers.find(peer=>peer.props.Checked)??peers[0];
      nextElements.get(node.key).control.tabIndex=target?.key===node.key?0:-1;
      if(node.kind==='Tab') {
        const panel=next.nodes.find(peer=>peer.props.Role==='tabpanel' && (peer.props.Group===node.props.Group || (peer.props.Group===undefined && peer.key===`${node.props.Group}-panel`)));
        attr(nextElements.get(node.key).control,'aria-controls',panel?nextElements.get(panel.key).element.id:null);
        if(panel && node.props.Checked) attr(nextElements.get(panel.key).element,'aria-labelledby',nextElements.get(node.key).element.id);
      }
    }
    applyTheme(next,nextElements);
    place(root,[nextElements.get(next.root).element]);
    elements=nextElements;bindings=nextBindings;revision++;
  }
  function render(candidate) {
    // Encode model before calling view so unsupported objects cannot mutate state.
    const encoded=argument(candidate),candidateModel=clone(candidate);
    const candidateRaw=app.view(encoded),next=validateView(candidateRaw);
    const candidateView=clone(candidateRaw), prepared=presentation.prepare(next,elements), preparedEffects=effects.prepare(next,elements), restore=snapshot(root), interactionFocus=document.activeElement, retain=retainInteraction(root);
    const previous={elements,bindings,revision,model,view,rawView};
    let overlaysAttempted=false,effectsAttempted=false;
    rendering=true;
    try {
      patch(next); retain();
      model=candidateModel;view=next;rawView=candidateView;
      overlaysAttempted=true;
      overlays.commit(next,elements,dispatchMessage,interactionFocus);
      effectsAttempted=true;
      effects.commit(next,elements,motionMode,dispatchMessage,preparedEffects);
      presentation.commit(prepared,elements);
    }
    catch(error) {
      ({elements,bindings,revision,model,view,rawView}=previous);
      try {
        restore();
        if(view){
          if(overlaysAttempted)overlays.commit(view,elements,dispatchMessage);
          if(effectsAttempted)effects.commit(view,elements,motionMode,dispatchMessage,undefined,{rollback:true});
        }
      } catch(fault) {broken=true;disconnect();report(new Error('Ubi UI rollback failed; remount required',{cause:fault}));}
      throw error;
    } finally {rendering=false;}
  }
  function resetControls() {
    for(const node of view.nodes) {
      const control=elements.get(node.key).control;
      if(editable.has(node.kind) && control.value!==node.props.Value) control.value=node.props.Value;
      if(['Checkbox','Switch','Radio'].includes(node.kind)) control.checked=node.props.Checked;
      if(node.kind==='Slider') control.value=String(node.props.Progress);
    }
  }
  function dispatch(binding,tag,value,sourceProp,allowClosedOverlay=false) {
    const node=binding.node,p=node.props,prop={Activate:'Activate',Input:'Input',Toggle:'Toggle',Select:'Change'}[tag];
    const messageProp=sourceProp??prop,eventTag=sourceProp?'Activate':tag;
    if(closed || broken || processing || rendering || owners.get(root)!==handle || binding.revision!==revision || !enabled(node) || (binding.control?.closest('[hidden],[inert]')&&!allowClosedOverlay) || (p.ReadOnly&&tag==='Input') || !Object.hasOwn(p,messageProp)) {resetControls();return;}
    processing=true;
    try {
      if(tag==='Input') {
        if(new TextEncoder().encode(value).length>16384) throw new TypeError('Input exceeds Ubi text limit');
        if(node.kind==='NumberInput' && value!=='' && value!=='-' && (!/^-?\d+$/.test(value) || !Number.isInteger(Number(value)) || Number(value)<-2147483648 || Number(value)>2147483647)) throw new TypeError('Expected 32-bit integer input');
      }
      const event={tag:eventTag,values:eventTag==='Activate'?[p[messageProp]]:[p[messageProp],value]};
      const candidate=app.update(argument(model),encode(event));
      render(candidate);
    } catch(error) {resetControls();report(error);}
    finally {processing=false;}
  }
  function dispatchMessage(source,prop,context) {
    const node=view?.nodes.find(item=>item.key===source.key&&item.kind===source.kind);
    if(!node||node.props[prop]===undefined)return;
    let allowClosedOverlay=false;
    if(prop==='OpenRequest') {
      if(context?.type!=='anchor-open'||node.kind!=='Overlay'||node.props.Open||!['Tooltip','ContextMenu'].includes(node.props.OverlayKind)||context.anchorKey!==node.props.Anchor)return;
      const anchor=view.nodes.find(item=>item.key===node.props.Anchor),entry=elements.get(node.props.Anchor);
      if(!anchor||!enabled(anchor)||!entry||!root.contains(entry.element)||!root.contains(entry.control)||entry.element.closest('[hidden],[inert]')||entry.control.closest('[hidden],[inert]'))return;
      allowClosedOverlay=true;
    }
    dispatch({node,revision,control:elements.get(node.key)?.control},'Activate',undefined,prop,allowClosedOverlay);
  }
  function bindingFor(event) {
    if(closed || broken || owners.get(root)!==handle) return null;
    const target=event.target;
    return root.contains(target)?bindings.get(target):null;
  }
  function activate(binding) {
    const {kind,props}=binding.node;
    if(['Toggle','Checkbox','Switch'].includes(kind)) dispatch(binding,'Toggle',!props.Checked);
    else if(kind==='OptionItem') {
      const input=overlays.comboboxInputForOption(binding.node);
      dispatch(binding,'Select',props.Option);
      if(input?.isConnected)input.focus({preventScroll:true});
    }
    else if(['Radio','Tab','ToggleItem','TableHeader'].includes(kind)) dispatch(binding,'Select',props.Option);
    else dispatch(binding,'Activate');
  }
  function click(event) {
    const target=event.target.closest?.('button'),binding=target&&bindings.get(target);
    if(binding && root.contains(target)) activate(binding);
  }
  function change(event) {
    const binding=bindingFor(event);
    if(binding && ['Checkbox','Switch','Radio'].includes(binding.node.kind)) activate(binding);
  }
  function input(event) {
    if(event.isComposing) return;
    const binding=bindingFor(event);if(!binding) return;
    if(editable.has(binding.node.kind)) dispatch(binding,'Input',event.target.value);
    else if(binding.node.kind==='Slider') dispatch(binding,'Select',String(Math.round(Number(event.target.value))));
  }
  function keydown(event) {
    const binding=bindingFor(event);if(!binding) return;
    const node=binding.node;
    if(node.kind==='TextInput'&&overlays.handleComboboxInputKeydown(event,node,option=>{
      const entry=elements.get(option.key),optionBinding=entry&&bindings.get(entry.control);
      if(optionBinding)activate(optionBinding);
    }))return;
    if(['MenuItem','OptionItem'].includes(node.kind)) {
      if(overlays.handleMenuKeydown(event,node))return;
      if(!event.repeat&&['Enter',' '].includes(event.key)){event.preventDefault();activate(binding);return;}
    }
    if(!isGroupItem(node) || !['ArrowLeft','ArrowRight','ArrowUp','ArrowDown','Home','End'].includes(event.key)) return;
    event.preventDefault();
    if(!enabled(node)) return;
    const peers=view.nodes.filter(peer=>peer.kind===node.kind && peer.props.Group===node.props.Group && enabled(peer));
    const index=peers.findIndex(peer=>peer.key===node.key),direction=['ArrowLeft','ArrowUp'].includes(event.key)?-1:1;
    const next=peers[event.key==='Home'?0:event.key==='End'?peers.length-1:(index+direction+peers.length)%peers.length];
    const control=elements.get(next.key).control;
    if(node.kind==='ToggleItem') peers.forEach(peer=>{elements.get(peer.key).control.tabIndex=peer.key===next.key?0:-1;});
    control.focus();
    if(node.kind!=='ToggleItem') activate(bindings.get(control));
  }
  const listeners={click,change,input,keydown,compositionend:input};
  function disconnect() {for(const [event,listener] of Object.entries(listeners)) root.removeEventListener(event,listener);media?.removeEventListener?.('change',onTheme);}
  const onTheme=()=>applyTheme();
  const handle={
    dispose() {if(closed)return;closed=true;disconnect();effects.dispose();overlays.dispose();presentation.dispose();bindings.clear();if(owners.get(root)===handle)owners.delete(root);},
    setMotion(mode) {if(!['system','full','reduced'].includes(mode))throw new TypeError('Unsupported Ubi motion mode');if(closed)return;motionMode=mode;presentation.setMotion(mode);effects.setMotion(mode);},
    setTheme(mode) {if(!['light','dark','system'].includes(mode))throw new TypeError('Unsupported Ubi theme');if(closed)return;theme=mode;applyTheme();},
    getModel:()=>clone(model),getView:()=>clone(rawView),
  };
  owners.set(root,handle);
  try {
    render(app.init());
    for(const [event,listener] of Object.entries(listeners)) root.addEventListener(event,listener);
    media?.addEventListener?.('change',onTheme);
  } catch(error) {handle.dispose();report(error);throw error;}
  return handle;
}
