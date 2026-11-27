import assert from 'node:assert/strict';
import { join } from 'node:path';

const a = await import('./dist/a.js');
const b = await import('./dist/b.js');
const namespace = await b.loaded;
assert.strictEqual(a.x, 1);
assert.strictEqual(namespace.x, 1);
assert.strictEqual(namespace.join, join);
