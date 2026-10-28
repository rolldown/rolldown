import assert from 'node:assert/strict';
import { readdirSync } from 'node:fs';
await assert.rejects(import('./dist/main.js'), /entry failed/);
const child = await globalThis.pendingChild;
assert.deepEqual(Object.keys(child), ['lazy']);
assert.equal(child.lazy.answer, 42);
assert.equal(
  readdirSync(new URL('./dist/', import.meta.url)).filter((file) => file.endsWith('.js')).length,
  3,
);
