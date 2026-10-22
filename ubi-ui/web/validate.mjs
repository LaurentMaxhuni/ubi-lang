import { encode } from './codec.mjs';

export const containers = new Set('Window View Column Row Flex Grid ScrollView Card'.split(' '));
export const editable = new Set('TextInput PasswordInput TextArea NumberInput'.split(' '));
export const interactive = new Set('Button IconButton Checkbox Switch Slider TextInput PasswordInput Radio Tab Toggle ToggleItem Link TextArea NumberInput'.split(' '));
const kinds = new Set([...containers, ...interactive, ...'Text Heading Separator Badge ProgressBar Avatar Alert Kbd Skeleton Label'.split(' ')]);
const roles = new Set('group radiogroup tablist tabpanel region alert status navigation list listitem button heading link text label'.split(' '));
const textProps = new Set('Content Label Value Option Group Role'.split(' '));
const boolProps = new Set('Disabled Busy Pressed Checked Selected ReadOnly Expanded Center Clip'.split(' '));
const sizeProps = new Set('Width Height WidthPercent HeightPercent MinWidth MinHeight MaxWidth MaxHeight Padding Grow Columns FontSize Progress Level'.split(' '));
const messageProps = new Set('Activate Input Toggle Change'.split(' '));
const variants = {Gap:'None Xs Sm Md Lg Xl Xxl',Tone:'Neutral Primary Success Warning Danger',Variant:'Primary Secondary Outline Ghost Destructive',Theme:'Light Dark System'};
const bytes = text => new TextEncoder().encode(text).length;
const fail = message => { throw new TypeError(message); };
function record(value, keys) {
  if (!value || Array.isArray(value) || typeof value !== 'object' || Object.keys(value).length !== keys.length || keys.some(key => !Object.hasOwn(value,key))) fail('Invalid Ubi record');
}
function tagged(value) { record(value,['tag','values']); if (typeof value.tag !== 'string' || !Array.isArray(value.values)) fail('Invalid Ubi enum'); }
function integer(value,min,max) { if (!Number.isSafeInteger(value) || value < min || value > max) fail('Invalid Ubi integer range'); }

/** Decode exactly the source-owned schema-1 View ABI. Never execute getters. */
export function validateView(input) {
  // Codec checks plain own-data records, Unicode, cycles, aggregate depth and bytes.
  const raw = JSON.parse(encode(input));
  record(raw,['schemaVersion','root','nodes']);
  if (raw.schemaVersion !== 1 || typeof raw.root !== 'string' || !Array.isArray(raw.nodes) || !raw.nodes.length || raw.nodes.length > 4096) fail('Unsupported Ubi view version or node count');
  const nodes = [], byKey = new Map();
  for (const item of raw.nodes) {
    record(item,['key','kind','props','children']); tagged(item.kind);
    if (typeof item.key !== 'string' || !item.key || bytes(item.key)>256 || /[\u0000-\u001f\u007f-\u009f]/u.test(item.key) || item.kind.values.length || !kinds.has(item.kind.tag) || !Array.isArray(item.props) || item.props.length>64 || !Array.isArray(item.children) || item.children.some(child=>typeof child!=='string')) fail('Invalid Ubi node');
    const kind = item.kind.tag;
    if (!containers.has(kind) && item.children.length) fail('Leaf components cannot own children');
    const props = {Content:'',Label:'',Value:'',Option:'',Group:'',Disabled:false,Busy:false,Pressed:false,Checked:false,ReadOnly:false,Variant:'Primary',Tone:'Neutral',Min:-2147483648,Max:2147483647,Step:1,Progress:0,Level:2};
    const style = {gap:kind==='Window'?24:kind==='Card'?16:12,padding:kind==='Window'?32:kind==='Card'?24:0,fontSize:kind==='Heading'?26:14,columns:2};
    const seen = new Set();
    for (const prop of item.props) {
      tagged(prop);
      const name = prop.tag, values = prop.values;
      if (seen.has(name)) fail(`Duplicate Ubi property: ${name}`); seen.add(name);
      if (name==='Absolute') {
        if (values.length!==2) fail('Absolute requires x,y');
        values.forEach(value=>integer(value,-100000,100000)); props.Absolute=values; continue;
      }
      if (values.length!==1) fail('Ubi property requires one value');
      const value = values[0];
      if (textProps.has(name)) {
        if (typeof value!=='string' || bytes(value)>16384) fail('Expected bounded text property');
        if (name==='Role' && !roles.has(value)) fail('Unsupported semantic role');
      } else if (boolProps.has(name)) {
        if (typeof value!=='boolean') fail('Expected bool property');
      } else if (sizeProps.has(name)) {
        integer(value,0,100000);
        if (['WidthPercent','HeightPercent','Progress'].includes(name) && value>100) fail('Percent must be 0..100');
        if (name==='Columns') integer(value,1,64);
        if (name==='FontSize') integer(value,8,96);
        if (name==='Level') integer(value,1,6);
      } else if (['Min','Max','Step'].includes(name)) integer(value,-2147483648,2147483647);
      else if (Object.hasOwn(variants,name)) {
        tagged(value);
        if (value.values.length || !variants[name].split(' ').includes(value.tag)) fail('Invalid enum variant');
      } else if (!messageProps.has(name)) fail(`Unknown Ubi property: ${name}`);
      props[name] = Object.hasOwn(variants,name) ? value.tag : value;
      if (name==='Level') style.fontSize=[32,26,22,18,16,14][value-1];
      if (name==='FontSize') style.fontSize=value;
      if (name==='Gap') style.gap={None:0,Xs:4,Sm:8,Md:12,Lg:16,Xl:24,Xxl:32}[value.tag];
      if (name==='Padding') style.padding=value;
      if (name==='Columns') style.columns=value;
      if (name==='Width' || name==='WidthPercent') style.width=`${value}${name==='Width'?'px':'%'}`;
      if (name==='Height' || name==='HeightPercent') style.height=`${value}${name==='Height'?'px':'%'}`;
    }
    if (props.Min>props.Max || props.Step<=0 || (props.MaxWidth!==undefined && props.MaxWidth<(props.MinWidth??0)) || (props.MaxHeight!==undefined && props.MaxHeight<(props.MinHeight??0))) fail('Invalid numeric range');
    if (seen.has('Checked') && seen.has('Selected')) fail('Checked and Selected conflict');
    if (seen.has('Selected')) props.Checked=props.Selected;
    if (kind==='IconButton' && !props.Label) fail('IconButton requires accessible Label');
    if (byKey.has(item.key)) fail('Duplicate node key');
    const node = {key:item.key,kind,props,style,children:item.children}; nodes.push(node); byKey.set(node.key,node);
  }
  const visited = new Set(), order = [];
  function visit(key,depth) {
    if (depth>32 || visited.has(key)) fail('Tree depth exceeded, cycle or multiple parents');
    const node=byKey.get(key); if (!node) fail('Missing child/root');
    visited.add(key); order.push(node); node.children.forEach(child=>visit(child,depth+1));
  }
  visit(raw.root,0);
  if (visited.size!==nodes.length) fail('Unreachable Ubi node');
  return {schemaVersion:1,root:raw.root,nodes:order};
}
