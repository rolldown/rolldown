import assert from 'node:assert/strict';
import { copyFile } from 'node:fs/promises';

await copyFile(
  new URL('./external.js', import.meta.url),
  new URL('./dist/external.js', import.meta.url),
);

globalThis.mergeExternalLog = [];
const main = await import('./dist/main.js');

assert.deepEqual(globalThis.mergeExternalLog, ['a', 'external', 'b', 'main']);
await main.loadA();
await main.loadB();
assert.deepEqual(globalThis.mergeExternalLog, ['a', 'external', 'b', 'main', 'route-a', 'route-b']);
