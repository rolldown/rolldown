import assert from 'node:assert/strict';
import { copyFile } from 'node:fs/promises';

await copyFile(
  new URL('./external.cjs', import.meta.url),
  new URL('./dist/external.cjs', import.meta.url),
);
assert.equal((await import('./dist/first.js')).default, 'external:ready');
