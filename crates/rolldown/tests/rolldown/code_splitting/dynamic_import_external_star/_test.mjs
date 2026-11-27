import assert from 'node:assert/strict';
import { join } from 'node:path';

const main = await import('./dist/main.js');
const namespace = await main.loaded;
assert.strictEqual(main.x, 1);
assert.strictEqual(namespace.x, 1);
assert.strictEqual(namespace.join, join);
