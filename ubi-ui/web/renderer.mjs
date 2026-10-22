import { encode } from './codec.mjs';
import { validateView, containers, editable, interactive } from './validate.mjs';
export { validateView } from './validate.mjs';

const owners = new WeakMap();
let nextMount = 0;
const clone = value => structuredClone(value);
// Public Ubi wrappers accept primitives directly and aggregates as JSON text.
export function argument(value) {
  const encoded = encode(value);
  return value !== null && typeof value === 'object' ? encoded : value;
}
const enabled = node => !node.props.Disabled && !node.props.Busy;
const isGroupItem = node => ['Radio','Tab','ToggleItem'].includes(node.kind) && node.props.Group;
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
  if(['Button','IconButton','Toggle','ToggleItem','Tab','Link'].includes(node.kind)) return 'button';
  return {Heading:`h${node.props.Level}`,Text:'p',Kbd:'kbd',Label:'span',Badge:'span',Avatar:'span',ProgressBar:'progress',Separator:'hr'}[node.kind]??'div';
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
  return {element,control,caption,kind:node.kind};
}
function styles(element,node) {
  const p=node.props,s=node.style,style=element.style;
  style.cssText='';
  style.gap=`${s.gap}px`; style.padding=`${s.padding}px`; style.fontSize=`${s.fontSize}px`;
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
  owners.get(root)?.dispose();
  const id=`ubi-web-${++nextMount}`,document=root.ownerDocument;
  const media=document.defaultView.matchMedia?.('(prefers-color-scheme: dark)');
  let theme=options.theme, model, view, rawView, elements=new Map(), bindings=new Map(), revision=0, closed=false, processing=false, broken=false;
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
      const entry=old?.kind===node.kind && old.element.localName===tagFor(node)?old:create(document,node);
      const {element,control,caption}=entry,p=node.props;
      if(!element.id) element.id=`${id}-${revision+1}-${i}`;
      element.className=`ubi-node ubi-${node.kind.toLowerCase()}`;
      element.setAttribute('data-ubi-key',node.key);
      element.hidden=Boolean(options.preview && next.nodes.find(item=>item.key===next.root)?.children.includes(node.key) && ['title','description'].includes(node.key));
      attr(element,'data-tone',p.Tone.toLowerCase()); attr(element,'data-variant',p.Variant.toLowerCase());
      attr(element,'data-checked',p.Checked?'true':null);
      attr(element,'data-disabled',enabled(node)?null:'true');
      styles(element,node);
      const role=p.Role??({Window:'region',Alert:'alert',Avatar:'img',Link:'link',Tab:'tab',Switch:'switch'}[node.kind]);
      attr(control,'role',role);
      attr(control,'aria-label',p.Label || (node.kind==='Avatar'?p.Content:null));
      attr(control,'aria-busy',p.Busy?'true':null);
      attr(control,'aria-expanded',p.Expanded);
      attr(control,'aria-disabled',!enabled(node)?'true':null);
      attr(control,'aria-readonly',p.ReadOnly?'true':null);
      attr(control,'aria-selected',node.kind==='Tab'?p.Checked:undefined);
      attr(control,'aria-pressed',['Toggle','ToggleItem'].includes(node.kind)?p.Checked:node.kind==='Button'&&p.Pressed?true:undefined);
      if('disabled' in control && interactive.has(node.kind)) control.disabled=!enabled(node);
      if(editable.has(node.kind)) {
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
      if(node.kind==='ProgressBar') {element.max=100;element.value=p.Progress;}
      if(caption && caption.textContent!==(p.Label||p.Content)) caption.textContent=p.Label||p.Content;
      if(!caption && !containers.has(node.kind) && node.kind!=='ProgressBar' && element.textContent!==p.Content) element.textContent=p.Content;
      if(node.kind==='Separator') attr(element,'aria-orientation','horizontal');
      if(interactive.has(node.kind)) nextBindings.set(control,{node,revision:revision+1});
      nextElements.set(node.key,entry);
    }
    for(const node of next.nodes) if(containers.has(node.kind)) place(nextElements.get(node.key).element,node.children.map(key=>nextElements.get(key).element));
    // Roving focus is scoped to this mount and source group identity.
    for(const node of next.nodes) {
      if(!isGroupItem(node)) continue;
      const peers=next.nodes.filter(peer=>peer.kind===node.kind && peer.props.Group===node.props.Group && enabled(peer));
      const target=peers.find(peer=>peer.props.Checked)??peers[0];
      nextElements.get(node.key).control.tabIndex=target?.key===node.key?0:-1;
      if(node.kind==='Tab') {
        const panel=next.nodes.find(peer=>peer.props.Role==='tabpanel' && peer.key===`${node.props.Group}-panel`);
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
    const candidateView=clone(candidateRaw), restore=snapshot(root), retain=retainInteraction(root);
    const previous={elements,bindings,revision};
    try { patch(next); retain(); }
    catch(error) {
      ({elements,bindings,revision}=previous);
      try {restore();} catch(fault) {broken=true;disconnect();report(new Error('Ubi UI rollback failed; remount required',{cause:fault}));}
      throw error;
    }
    model=candidateModel;view=next;rawView=candidateView;
  }
  function resetControls() {
    for(const node of view.nodes) {
      const control=elements.get(node.key).control;
      if(editable.has(node.kind) && control.value!==node.props.Value) control.value=node.props.Value;
      if(['Checkbox','Switch','Radio'].includes(node.kind)) control.checked=node.props.Checked;
      if(node.kind==='Slider') control.value=String(node.props.Progress);
    }
  }
  function dispatch(binding,tag,value) {
    const node=binding.node,p=node.props,prop={Activate:'Activate',Input:'Input',Toggle:'Toggle',Select:'Change'}[tag];
    if(closed || broken || processing || owners.get(root)!==handle || binding.revision!==revision || !enabled(node) || (p.ReadOnly&&tag==='Input') || !Object.hasOwn(p,prop)) {resetControls();return;}
    processing=true;
    try {
      if(tag==='Input') {
        if(new TextEncoder().encode(value).length>16384) throw new TypeError('Input exceeds Ubi text limit');
        if(node.kind==='NumberInput' && value!=='' && value!=='-' && (!/^-?\d+$/.test(value) || !Number.isInteger(Number(value)) || Number(value)<-2147483648 || Number(value)>2147483647)) throw new TypeError('Expected 32-bit integer input');
      }
      const event={tag,values:tag==='Activate'?[p[prop]]:[p[prop],value]};
      const candidate=app.update(argument(model),encode(event));
      render(candidate);
    } catch(error) {resetControls();report(error);}
    finally {processing=false;}
  }
  function bindingFor(event) {
    if(closed || broken || owners.get(root)!==handle) return null;
    const target=event.target;
    return root.contains(target)?bindings.get(target):null;
  }
  function activate(binding) {
    const {kind,props}=binding.node;
    if(['Toggle','Checkbox','Switch'].includes(kind)) dispatch(binding,'Toggle',!props.Checked);
    else if(['Radio','Tab','ToggleItem'].includes(kind)) dispatch(binding,'Select',props.Option);
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
    dispose() {if(closed)return;closed=true;disconnect();bindings.clear();if(owners.get(root)===handle)owners.delete(root);},
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
