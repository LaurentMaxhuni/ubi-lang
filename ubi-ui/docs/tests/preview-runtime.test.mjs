import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, copyFileSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname, resolve, basename } from 'node:path';
import test from 'node:test';
import { previewRuntimeSources, verifyPreviewRuntime } from '../../tools/verify_preview_runtime.mjs';

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
