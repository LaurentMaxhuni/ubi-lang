import { existsSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
const platform = process.argv[2];
if (!['android', 'ios'].includes(platform)) {
  console.error('Usage: node native.mjs android|ios');
  process.exit(2);
}
if (platform === 'ios' && process.platform !== 'darwin') {
  console.error('iOS generation/build requires macOS and Xcode 26+');
  process.exit(2);
}
if (!existsSync('www/index.html')) {
  console.error('Missing www/index.html; run ubi package --target mobile first');
  process.exit(2);
}
const cli = fileURLToPath(new URL('./node_modules/@capacitor/cli/bin/capacitor', import.meta.url));
if (!existsSync(cli)) {
  console.error('Missing Capacitor dependencies; run npm ci');
  process.exit(2);
}
for (const args of existsSync(platform) ? [['sync', platform]] : [['add', platform], ['sync', platform]]) {
  const result = spawnSync(process.execPath, [cli, ...args], { stdio: 'inherit' });
  if (result.error) { console.error(result.error.message); process.exit(2); }
  if (result.status !== 0) process.exit(result.status ?? 2);
}
