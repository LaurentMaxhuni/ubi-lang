import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { encode } from '../../examples/ui/web/codec.mjs';
import { argument } from '../web/renderer.mjs';
const fixtures = JSON.parse(fs.readFileSync(0, 'utf8'));
const root = process.argv[2];
let events = 0;
for (const fixture of fixtures) {
  const app = await import(pathToFileURL(path.join(root, fixture.module)));
  const initial = app.init();
  assert.deepEqual(initial, fixture.initial, `${fixture.slug}: initial model`);
  assert.deepEqual(app.view(argument(initial)), fixture.view, `${fixture.slug}: initial view`);
  for (const { event, model, view } of fixture.cases) {
    const next = app.update(argument(initial), encode(event));
    assert.deepEqual(next, model, `${fixture.slug}: ${event.tag} model`);
    assert.deepEqual(app.view(argument(next)), view, `${fixture.slug}: ${event.tag} view`);
    events++;
  }
}
assert.equal(fixtures.length, 41);
assert.ok(events >= 30, 'representative controlled event coverage');
console.log(`${fixtures.length} real Ubi browser apps match native execution; ${events} event transitions`);
