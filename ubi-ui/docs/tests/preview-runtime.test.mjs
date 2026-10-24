import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, copyFileSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname, resolve, basename } from 'node:path';
import test from 'node:test';
import { previewRuntimeSources, verifyPreviewRuntime, verifyPreviewAssets } from '../../tools/verify_preview_runtime.mjs';

test('preview verification rejects a stale renderer even when another copy is current', () => {
  const previews = mkdtempSync(join(tmpdir(), 'ubi-preview-runtime-'));
  try {
    mkdirSync(join(previews, 'runtime'));
    for (const [file, source] of previewRuntimeSources) copyFileSync(source, join(previews, 'runtime', file));
    verifyPreviewRuntime(previews);
    copyFileSync(previewRuntimeSources.get('renderer.mjs'), join(previews, 'renderer.mjs'));
    writeFileSync(join(previews, 'runtime', 'renderer.mjs'), '// stale runtime');
    assert.throws(() => verifyPreviewRuntime(previews), /Stale preview runtime: .*runtime.*renderer.mjs/);
  } finally {
    assert.equal(dirname(resolve(previews)), resolve(tmpdir()));
    assert.ok(basename(previews).startsWith('ubi-preview-runtime-'));
    rmSync(previews, { recursive: true, force: true });
  }
});

test('preview verification follows application re-exports and rejects missing imported modules',()=>{
  const previews=mkdtempSync(join(tmpdir(),'ubi-preview-runtime-'));
  try{
    mkdirSync(join(previews,'select'));
    writeFileSync(join(previews,'manifest.json'),JSON.stringify([{module:'select/app.mjs'}]));
    writeFileSync(join(previews,'select/app.mjs'),'export { init, update, view } from "./missing.mjs";\n');
    assert.throws(()=>verifyPreviewAssets(previews),/Missing preview module: .*missing.mjs/);
    writeFileSync(join(previews,'select/missing.mjs'),'export function init() {}');
    assert.equal(verifyPreviewAssets(previews),2);
  }finally{
    assert.equal(dirname(resolve(previews)),resolve(tmpdir()));assert.ok(basename(previews).startsWith('ubi-preview-runtime-'));
    rmSync(previews,{recursive:true,force:true});
  }
});
