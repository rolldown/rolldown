import assert from 'node:assert/strict';
import { copyFile } from 'node:fs/promises';

await copyFile(
  new URL('./external.mjs', import.meta.url),
  new URL('./dist/external.mjs', import.meta.url),
);
await assert.rejects(import('./dist/lib.js'), ReferenceError);
