import assert from 'node:assert/strict';
import { join } from 'node:path';

const live = await import('./dist/live.js');
const main = await import('./dist/main.js');
const namespace = await main.loaded;
assert.strictEqual(live.join, join);
assert.strictEqual(namespace.x, 1);
assert.strictEqual(namespace.join, join);
