import assert from 'node:assert/strict';
import fs from 'node:fs';

globalThis.takesNothingLog = [];
const main = await import('./dist/main.js');

assert.deepEqual(globalThis.takesNothingLog, ['shared', 'main:shared']);
assert.equal((await main.load()).dynamic, 'shared');
assert.deepEqual(
  fs.readdirSync(new URL('./dist', import.meta.url)).filter((file) => file.startsWith('shared')),
  [],
);
