import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

for (const entry of ['other.js', 'index.js']) {
  const path = fileURLToPath(new URL(`./dist/${entry}`, import.meta.url));
  const result = spawnSync(process.execPath, [path], { encoding: 'utf8' });
  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(result.stdout.trim().split(/\r?\n/), ['setup', 'lib', 'after']);
}
