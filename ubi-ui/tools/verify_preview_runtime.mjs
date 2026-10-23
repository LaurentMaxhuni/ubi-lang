import { readFileSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
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

if (process.argv[1] && pathToFileURL(resolve(process.argv[1])).href === import.meta.url) {
  verifyPreviewRuntime(resolve(process.argv[2] ?? 'public/previews'));
  console.log('Preview runtime matches current source.');
}
