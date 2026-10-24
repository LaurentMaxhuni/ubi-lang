import { readFileSync } from 'node:fs';
import { resolve, dirname, relative, isAbsolute } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const tools = dirname(fileURLToPath(import.meta.url));
export const previewRuntimeSources = new Map([
  ...['renderer.mjs', 'renderer.css', 'validate.mjs', 'motion.mjs', 'presentation.mjs', 'overlays.mjs', 'effects.mjs'].map(file => [file, resolve(tools, '../web', file)]),
  ['codec.mjs', resolve(tools, '../../examples/ui/web/codec.mjs')],
  ['boot.mjs', resolve(tools, 'web_boot.mjs')],
  ['shell.css', resolve(tools, 'web_shell.css')],
]);

export function verifyPreviewRuntime(previews) {
  for (const [file, source] of previewRuntimeSources) {
    const exported = resolve(previews, 'runtime', file);
    if (!readFileSync(source).equals(readFileSync(exported))) {
      throw new Error(`Stale preview runtime: ${exported}. Regenerate with npm run examples before building docs.`);
    }
  }
}

export function verifyPreviewAssets(previews) {
  const root=resolve(previews),manifest=JSON.parse(readFileSync(resolve(root,'manifest.json'),'utf8')),visited=new Set();
  const pending=manifest.map(app=>resolve(root,app.module));
  while(pending.length){
    const file=pending.pop(),path=relative(root,file);
    if(isAbsolute(path)||path==='..'||path.startsWith('..\\')||path.startsWith('../'))throw new Error(`Preview import escapes output directory: ${file}`);
    if(visited.has(file))continue;visited.add(file);
    let source;try{source=readFileSync(file,'utf8');}catch{throw new Error(`Missing preview module: ${file}`);}
    for(const match of source.matchAll(/^(?:import|export)\s+.+?\s+from\s+["']([^"']+)["'];?\s*$/gm)){
      if(!match[1].startsWith('.'))throw new Error(`Unbundled preview dependency: ${match[1]}`);
      pending.push(resolve(dirname(file),match[1]));
    }
  }
  return visited.size;
}

if (process.argv[1] && pathToFileURL(resolve(process.argv[1])).href === import.meta.url) {
  verifyPreviewRuntime(resolve(process.argv[2] ?? 'public/previews'));
  const count=verifyPreviewAssets(resolve(process.argv[2]??'public/previews'));
  console.log(`Preview runtime matches current source; ${count} module assets verified.`);
}
