import assert from 'node:assert/strict';
import { copyFile } from 'node:fs/promises';

await copyFile(
  new URL('./external.js', import.meta.url),
  new URL('./dist/external.js', import.meta.url),
);

const main = await import('./dist/main.js');

assert.equal(globalThis.externalSawReady, true);
await main.loadRoute();
assert.equal(await main.loadSplash(), 'serviceslug');
assert.equal(await main.loadBarrel(), 'service');
