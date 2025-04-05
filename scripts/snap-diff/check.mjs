import { spawnSync } from 'node:child_process';
console.log(`process.platform: `, process.platform);

if (process.platform === 'win32') {
  process.exit(0);
}
const res = spawnSync('git diff --exit-code', {
  shell: true,
  stdio: 'inherit',
  cwd: process.cwd(),
});
process.exit(res.status ?? 0);
