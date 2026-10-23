import assert from 'node:assert/strict';
import fs from 'node:fs';
import { validateView } from '../web/validate.mjs';

const cases = JSON.parse(fs.readFileSync(0, 'utf8'));
for (const { name, view, valid } of cases) {
  let accepted = true;
  try { validateView(view); } catch { accepted = false; }
  assert.equal(accepted, valid, `browser ABI: ${name}`);
}
console.log(`${cases.length} shared view contract cases agree`);
