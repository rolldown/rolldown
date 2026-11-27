import assert from 'node:assert/strict';
import { join } from 'node:path';

const live = await import('./dist/live.js');
const main = await import('./dist/main.js');
const namespace = await main.loaded;
assert.strictEqual(live.fromX, 7);
assert.strictEqual(live.join, join);
assert.strictEqual(namespace.viaBarrel, 7);
assert.strictEqual(namespace.join, join);
