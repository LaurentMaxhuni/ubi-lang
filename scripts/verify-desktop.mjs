// Verify the actual packaged executable, with isolated disposable app storage.
import { stat } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { spawnSync } from 'node:child_process';

const output = resolve(process.argv[2] ?? 'examples/v1/.ubi-dist/desktop/out');
const directory = join(output, `UbiTasks-${process.platform}-${process.arch}`);
const executable = process.platform === 'win32'
  ? join(directory, 'UbiTasks.exe')
  : process.platform === 'darwin'
    ? join(directory, 'UbiTasks.app', 'Contents', 'MacOS', 'UbiTasks')
    : join(directory, 'UbiTasks');
if (!(await stat(executable)).isFile()) throw new Error(`Missing desktop executable: ${executable}`);
const result = spawnSync(executable, ['--smoke-test'], { encoding: 'utf8', timeout: 120_000, windowsHide: true });
if (result.stdout) process.stdout.write(result.stdout);
if (result.stderr) process.stderr.write(result.stderr);
if (result.error) throw result.error;
if (result.status !== 0 || !result.stdout?.includes('UBI_DESKTOP_SMOKE_OK')) {
  throw new Error(`Packaged desktop verification failed (exit ${result.status}, signal ${result.signal})`);
}
