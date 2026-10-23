import { encode } from './codec.mjs';

export const containers = new Set('Window View Column Row Flex Grid ScrollView Card Accordion Collapsible Overlay EffectSurface Pagination Table TableRow TableHeader TableCell'.split(' '));
export const editable = new Set('TextInput PasswordInput TextArea NumberInput'.split(' '));
export const interactive = new Set('Button IconButton Checkbox Switch Slider TextInput PasswordInput Radio Tab Toggle ToggleItem Link TextArea NumberInput MenuItem OptionItem'.split(' '));
const kinds = new Set([...containers, ...interactive, ...'Text Heading Separator Badge ProgressBar Avatar Alert Kbd Skeleton Label'.split(' ')]);
const roles = new Set('group radiogroup tablist tab tabpanel region alert alertdialog status navigation list listitem listbox option combobox menu menubar menuitem menuitemcheckbox menuitemradio button heading link text label dialog tooltip table row columnheader cell grid gridcell checkbox switch progressbar separator toolbar search'.split(' '));
const textProps = new Set('Content Label Value Option Group Role Anchor LabelledBy DescribedBy'.split(' '));
const boolProps = new Set('Disabled Busy Pressed Checked Selected ReadOnly Expanded Center Clip Open Disclosure AutoPlay Paused'.split(' '));
const sizeProps = new Set('Width Height WidthPercent HeightPercent MinWidth MinHeight MaxWidth MaxHeight Padding Grow Columns FontSize Progress Level Opacity Scale Index Page PageCount TimeoutMs PeriodMs'.split(' '));
const messageProps = new Set('Activate Input Toggle Change Advance Dismiss OpenRequest'.split(' '));
const variants = {
  Gap:'None Xs Sm Md Lg Xl Xxl', Tone:'Neutral Primary Success Warning Danger',
  Variant:'Primary Secondary Outline Ghost Destructive', Theme:'Light Dark System',
  Enter:'None Fade Rise Scale', Exit:'None Fade Rise Scale',
  OverlayKind:'Dialog AlertDialog Drawer Popover Tooltip Toast Select Combobox DropdownMenu ContextMenu CommandPalette',
  Placement:'Top Right Bottom Left Center',
  Effect:'NumberTicker TextReveal AnimatedList Shimmer SpotlightCard TiltCard CardStack Carousel Dock Marquee RotatingText BorderBeam AuroraBackground GridBackground ScrollReveal',
};
const bytes = text => new TextEncoder().encode(text).length;
const fail = message => { throw new TypeError(message); };
function record(value, keys) {
  if (!value || Array.isArray(value) || typeof value !== 'object' || Object.keys(value).length !== keys.length || keys.some(key => !Object.hasOwn(value,key))) fail('Invalid Ubi record');
}
function tagged(value) { record(value,['tag','values']); if (typeof value.tag !== 'string' || !Array.isArray(value.values)) fail('Invalid Ubi enum'); }
function integer(value,min,max) { if (!Number.isSafeInteger(value) || value < min || value > max) fail('Invalid Ubi integer range'); }
function enumValue(value, options) {
  tagged(value);
  if (value.values.length || !options.split(' ').includes(value.tag)) fail('Invalid enum variant');
  return value.tag;
}
function motionValue(value) {
  record(value,['durationMs','delayMs','easing']);
  integer(value.durationMs,0,2000); integer(value.delayMs,0,1000);
  return { durationMs:value.durationMs, delayMs:value.delayMs, easing:enumValue(value.easing,'Linear EaseOut EaseInOut Spring') };
}
function messageValue(value) { encode(value); return value; }
const modalKinds = new Set(['Dialog','AlertDialog','Drawer','CommandPalette']);
const anchoredKinds = new Set(['Popover','Tooltip','Select','Combobox','DropdownMenu','ContextMenu']);
const tooltipFocusableKinds = new Set('Button IconButton Checkbox Switch Slider TextInput PasswordInput Radio Tab Toggle ToggleItem Link TextArea NumberInput MenuItem OptionItem'.split(' '));
const defaultPlacement = kind => kind === 'Drawer' ? 'Right' : modalKinds.has(kind) || kind === 'Toast' ? 'Center' : 'Bottom';

function validateNodeProperties(node, seen) {
  const {kind,props:p}=node;
  const has=name=>seen.has(name);
  if (has('OverlayKind') !== (kind === 'Overlay')) fail('OverlayKind is required only on Overlay');
  if (kind !== 'Overlay' && ['Open','Anchor','Placement','Dismiss','TimeoutMs','OpenRequest'].some(has)) fail('Overlay property on non-Overlay node');
  if (has('TimeoutMs') && p.OverlayKind !== 'Toast') fail('TimeoutMs is valid only on Toast');
  if (has('OpenRequest') && !['Tooltip','ContextMenu'].includes(p.OverlayKind)) fail('OpenRequest is valid only on Tooltip or ContextMenu');
  if (has('Effect')) {
    if (!((kind === 'EffectSurface' && p.Effect !== 'Shimmer') || (kind === 'Button' && p.Effect === 'Shimmer'))) fail('Effect is valid only on EffectSurface or Button.Shimmer');
  } else if (kind === 'EffectSurface') fail('EffectSurface requires Effect');
  if (has('Number') !== (p.Effect === 'NumberTicker')) fail('Number is required only for NumberTicker');
  if (has('Index') && !['CardStack','Carousel','RotatingText'].includes(p.Effect)) fail('Index is valid only for CardStack, Carousel or RotatingText');
  const rotating=p.Effect==='RotatingText',marquee=p.Effect==='Marquee';
  if ((has('AutoPlay')||has('PeriodMs')||has('Advance'))&&!rotating) fail('Rotation property on non-RotatingText node');
  if (has('Paused')&&!rotating&&!marquee) fail('Paused is valid only for RotatingText or Marquee');
  if (rotating&&p.AutoPlay&& !has('Advance')) fail('AutoPlay requires Advance');
  if (has('Disclosure')&&!containers.has(kind)) fail('Disclosure is valid only on containers');
  if ((has('Page')||has('PageCount')) !== (kind==='Pagination') || (kind==='Pagination'&&(!has('Page')||!has('PageCount')))) fail('Pagination requires Page and PageCount');
  if (kind==='Pagination'&&p.Page>p.PageCount) fail('Page cannot exceed PageCount');
  if (kind==='Overlay') p.Placement ??= defaultPlacement(p.OverlayKind);
  if (['MenuItem','OptionItem'].includes(kind) && (!p.Option || !p.Label)) fail('Menu options require Label and Option');
  if (kind==='MenuItem'&&!has('Activate')) fail('MenuItem requires Activate');
  if (kind==='OptionItem'&&!has('Change')) fail('OptionItem requires Change');
  if (kind==='TableHeader'&&has('Change')&&(!p.Option||!p.Label)) fail('Sortable table headers require Label and Option');
}

function validateRelationships(nodes, byKey) {
  const parent = new Map();
  for (const node of nodes) for (const key of node.children) parent.set(key,node);
  const anchorEdges=new Map();
  for (const node of nodes) {
    for (const prop of ['LabelledBy','DescribedBy']) if (node.props[prop] !== undefined) {
      const key=node.props[prop]; if (!key || key===node.key || !byKey.has(key)) fail(`Invalid ${prop} reference`);
    }
    const anchor=node.props.Anchor;
    if(anchor!==undefined) {
      if(!anchor||anchor===node.key||!byKey.has(anchor))fail('Invalid Anchor reference');
      if(node.kind==='Overlay'&&anchoredKinds.has(node.props.OverlayKind)&&byKey.get(anchor).kind==='Overlay')anchorEdges.set(node.key,anchor);
    }
    if (node.kind !== 'Overlay') continue;
    const {OverlayKind:kind,Label:label,Content:content,LabelledBy:labelledBy,OpenRequest:request}=node.props;
    if (modalKinds.has(kind) && !(label??'').trim() && !labelledBy) fail('Modal overlay requires Label or LabelledBy');
    if (anchoredKinds.has(kind) && anchor===undefined) fail('Anchored overlay requires a valid Anchor');
    if (kind==='Tooltip') {
      if(!(label??'').trim()&&!(content??'').trim())fail('Tooltip requires a description');
      const subtreeHasFocusable=key=>{
        const child=byKey.get(key);if(!child)return false;
        return tooltipFocusableKinds.has(child.kind)||(child.kind==='TableHeader'&&child.props.Change!==undefined)||child.children.some(subtreeHasFocusable);
      };
      if(node.children.some(subtreeHasFocusable))fail('Tooltip cannot contain focusable content');
      if(nodes.some(candidate=>candidate.props.DescribedBy===node.key&&candidate.key!==anchor))fail('Tooltip description must reference its anchor');
    }
    if (request !== undefined && !anchor) fail('OpenRequest requires Anchor');
  }
  const activeAnchors=new Set(),checkedAnchors=new Set();
  const visitAnchor=key=>{
    if(checkedAnchors.has(key))return;
    if(activeAnchors.has(key))fail('Cyclic overlay anchors');
    activeAnchors.add(key);
    const next=anchorEdges.get(key);if(next)visitAnchor(next);
    activeAnchors.delete(key);checkedAnchors.add(key);
  };
  for(const key of anchorEdges.keys())visitAnchor(key);
  const owners = new Map();
  for (const node of nodes.filter(item=>['MenuItem','OptionItem'].includes(item.kind))) {
    let owner=parent.get(node.key);
    while(owner && owner.kind!=='Overlay') owner=parent.get(owner.key);
    const key=owner?.key??parent.get(node.key)?.key??'root';
    const values=owners.get(key)??new Set();
    if (values.has(node.props.Option)) fail('Duplicate menu option value');
    values.add(node.props.Option); owners.set(key,values);
  }
  for (const node of nodes.filter(item=>item.kind==='EffectSurface')) {
    const effect=node.props.Effect;
    if (['NumberTicker','TextReveal'].includes(effect)&&node.children.length) fail('Leaf effect cannot own children');
    if (effect==='RotatingText'&&!node.children.length) fail('RotatingText requires phrases');
    if (['CardStack','Carousel','RotatingText'].includes(effect) && (node.children.length ? node.props.Index>=node.children.length : node.props.Index!==0)) fail('Effect Index is outside its child range');
  }
  validateTables(nodes,byKey);
}

function validateTables(nodes,byKey) {
  for (const table of nodes.filter(node=>node.kind==='Table')) {
    const rows=table.children.map(key=>byKey.get(key));
    if (!rows.length || rows.some(row=>row.kind!=='TableRow')) fail('Table children must be TableRow nodes');
    const headerRows=rows.filter(row=>row.children.some(key=>byKey.get(key)?.kind==='TableHeader'));
    if (headerRows.length!==1 || rows[0]!==headerRows[0]) fail('Table requires one leading header row');
    const headers=headerRows[0].children.map(key=>byKey.get(key));
    if (!headers.length || headers.length>64 || headers.some(cell=>cell.kind!=='TableHeader')) fail('Invalid table header width');
    const columns=new Set();
    for(const header of headers) {
      if(!header.props.Option || columns.has(header.props.Option)) fail('Table column keys must be nonempty and unique');
      columns.add(header.props.Option);
    }
    const rowKeys=new Set();
    for(const row of rows.slice(1)) {
      if(!row.props.Option || rowKeys.has(row.props.Option)) fail('Table row keys must be nonempty and unique');
      rowKeys.add(row.props.Option);
      const cells=row.children.map(key=>byKey.get(key));
      if(cells.length!==headers.length || cells.some((cell,index)=>cell.kind!=='TableCell'||cell.props.Option!==headers[index].props.Option)) fail('Table row width or column keys do not match headers');
    }
  }
}

/** Decode the source-owned schema-1 View ABI and reject malformed relationships before DOM changes. */
export function validateView(input) {
  const raw=JSON.parse(encode(input));
  record(raw,['schemaVersion','root','nodes']);
  if(raw.schemaVersion!==1||typeof raw.root!=='string'||!Array.isArray(raw.nodes)||!raw.nodes.length||raw.nodes.length>4096) fail('Unsupported Ubi view version or node count');
  const nodes=[],byKey=new Map();
  for(const item of raw.nodes) {
    record(item,['key','kind','props','children']); tagged(item.kind);
    if(typeof item.key!=='string'||!item.key||bytes(item.key)>256||/[\u0000-\u001f\u007f-\u009f]/u.test(item.key)||item.kind.values.length||!kinds.has(item.kind.tag)||!Array.isArray(item.props)||item.props.length>64||!Array.isArray(item.children)||item.children.some(child=>typeof child!=='string')) fail('Invalid Ubi node');
    const kind=item.kind.tag;
    if(!containers.has(kind)&&item.children.length) fail('Leaf components cannot own children');
    const props={Content:'',Label:'',Value:'',Option:'',Group:'',Disabled:false,Busy:false,Pressed:false,Checked:false,ReadOnly:false,Variant:'Primary',Tone:'Neutral',Min:-2147483648,Max:2147483647,Step:1,Progress:0,Level:2,Opacity:100,Translate:[0,0],Scale:100,Open:false,AutoPlay:false,Paused:false,Index:0};
    const style={gap:kind==='Window'?24:kind==='Card'?16:12,padding:kind==='Window'?32:kind==='Card'?24:0,fontSize:kind==='Heading'?26:14,columns:2};
    const seen=new Set();
    for(const prop of item.props) {
      tagged(prop); const name=prop.tag,values=prop.values;
      if(seen.has(name)) fail(`Duplicate Ubi property: ${name}`); seen.add(name);
      if(name==='Absolute'||name==='Translate') {
        if(values.length!==2) fail(`${name} requires two values`);
        values.forEach(value=>integer(value,-100000,100000)); props[name]=values;
        if(name==='Translate') { props.Translate=values; }
        continue;
      }
      if(values.length!==1) fail('Ubi property requires one value');
      const value=values[0];
      if(textProps.has(name)) {
        if(typeof value!=='string'||bytes(value)>16384) fail('Expected bounded text property');
        if(name==='Role'&&!roles.has(value)) fail('Unsupported semantic role');
      } else if(boolProps.has(name)) {
        if(typeof value!=='boolean') fail('Expected bool property');
      } else if(sizeProps.has(name)) {
        const range=name==='Opacity'?[0,100]:name==='Scale'?[25,200]:name==='Index'?[0,4095]:name==='TimeoutMs'?[0,60000]:name==='PeriodMs'?[250,60000]:['Page','PageCount'].includes(name)?[1,100000]:[0,100000];
        integer(value,...range);
        if(['WidthPercent','HeightPercent','Progress'].includes(name)&&value>100) fail('Percent must be 0..100');
        if(name==='Columns') integer(value,1,64);
        if(name==='FontSize') integer(value,8,96);
        if(name==='Level') integer(value,1,6);
      } else if(['Min','Max','Step','Number'].includes(name)) integer(value,-2147483648,2147483647);
      else if(name==='Motion') props.Motion=motionValue(value);
      else if(Object.hasOwn(variants,name)) props[name]=enumValue(value,variants[name]);
      else if(messageProps.has(name)) props[name]=messageValue(value);
      else fail(`Unknown Ubi property: ${name}`);
      if(Object.hasOwn(variants,name)) props[name]=enumValue(value,variants[name]);
      else if(name!=='Motion') props[name]=value;
      if(name==='Level') style.fontSize=[32,26,22,18,16,14][value-1];
      if(name==='FontSize') style.fontSize=value;
      if(name==='Gap') style.gap={None:0,Xs:4,Sm:8,Md:12,Lg:16,Xl:24,Xxl:32}[props.Gap];
      if(name==='Padding') style.padding=value;
      if(name==='Columns') style.columns=value;
      if(name==='Width'||name==='WidthPercent') style.width=`${value}${name==='Width'?'px':'%'}`;
      if(name==='Height'||name==='HeightPercent') style.height=`${value}${name==='Height'?'px':'%'}`;
    }
    if(props.Min>props.Max||props.Step<=0||(props.MaxWidth!==undefined&&props.MaxWidth<(props.MinWidth??0))||(props.MaxHeight!==undefined&&props.MaxHeight<(props.MinHeight??0))) fail('Invalid numeric range');
    if(seen.has('Checked')&&seen.has('Selected')) fail('Checked and Selected conflict');
    if(seen.has('Selected')) props.Checked=props.Selected;
    if(kind==='IconButton'&&!props.Label) fail('IconButton requires accessible Label');
    if(byKey.has(item.key)) fail('Duplicate node key');
    const node={key:item.key,kind,props,style,children:item.children};
    validateNodeProperties(node,seen);
    nodes.push(node); byKey.set(node.key,node);
  }
  const visited=new Set(),order=[];
  function visit(key,depth) {
    if(depth>32||visited.has(key)) fail('Tree depth exceeded, cycle or multiple parents');
    const node=byKey.get(key); if(!node) fail('Missing child/root');
    visited.add(key); order.push(node); node.children.forEach(child=>visit(child,depth+1));
  }
  visit(raw.root,0);
  if(visited.size!==nodes.length) fail('Unreachable Ubi node');
  validateRelationships(order,byKey);
  return {schemaVersion:1,root:raw.root,nodes:order};
}
