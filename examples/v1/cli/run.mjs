import * as domain from './modules/src/main.mjs';
import { constants } from 'node:fs';
import { open, readFile, rename, unlink, lstat } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { resolve } from 'node:path';
import { randomUUID } from 'node:crypto';

function unwrap(result) {
  if (result.tag !== 'Ok') throw new Error(`Task validation failed: ${result.values[0].tag}`);
  return result.values[0];
}
async function regularFile(path) {
  try {
    const info = await lstat(path);
    if (!info.isFile() || info.isSymbolicLink()) throw new Error('Task store must be a regular file');
  } catch (cause) { if (cause.code !== 'ENOENT') throw cause; }
}
async function save(path, tasks) {
  await regularFile(path);
  const temporary = `${path}.${randomUUID()}.tmp`;
  let file;
  try {
    file = await open(temporary, constants.O_CREAT | constants.O_EXCL | constants.O_WRONLY, 0o600);
    await file.writeFile(JSON.stringify(tasks) + '\n', 'utf8');
    await file.sync();
    await file.close();
    file = undefined;
    await rename(temporary, path);
  } finally {
    await file?.close();
    await unlink(temporary).catch(cause => { if (cause.code !== 'ENOENT') throw cause; });
  }
}
try {
  const args = process.argv.slice(2);
  let store = fileURLToPath(new URL('./tasks.json', import.meta.url));
  const flag = args.indexOf('--store');
  if (flag !== -1) {
    if (!args[flag + 1]) throw new Error('--store requires a path');
    store = resolve(args[flag + 1]);
    args.splice(flag, 2);
  }
  const command = args.shift() ?? 'list';
  const lock = `${store}.lock`;
  const mutation = ['add', 'complete', 'delete'].includes(command);
  let handle;
  try {
    if (mutation) handle = await open(lock, constants.O_CREAT | constants.O_EXCL | constants.O_WRONLY, 0o600);
    await regularFile(store);
    let saved = '[]';
    try { saved = await readFile(store, 'utf8'); }
    catch (cause) { if (cause.code !== 'ENOENT') throw cause; }
    const tasks = unwrap(domain.decodeTasks(saved));
    let next;
    if (command === 'list' && args.length <= 1 && ['all', 'active', 'completed'].includes(args[0] ?? 'all')) {
      console.log(JSON.stringify(domain.filterTasks(JSON.stringify(tasks), args[0] ?? 'all')));
    } else if (command === 'add' && args.length > 0) {
      next = unwrap(domain.create(JSON.stringify(tasks), domain.nextId(JSON.stringify(tasks)), args.join(' ')));
    } else if (['complete', 'delete'].includes(command) && args.length === 1 && /^[1-9][0-9]*$/.test(args[0])) {
      const operation = command === 'complete' ? domain.complete : domain.deleteTask;
      next = unwrap(operation(JSON.stringify(tasks), Number(args[0])));
    } else {
      throw new Error('Usage: node run.mjs [--store path] list [all|active|completed] | add title | complete id | delete id');
    }
    if (next) { await save(store, next); console.log(JSON.stringify(next)); }
  } finally {
    if (handle) { await handle.close(); await unlink(lock); }
  }
} catch (cause) {
  console.error(`Ubi Tasks: ${cause.message ?? cause}`);
  process.exitCode = 1;
}
