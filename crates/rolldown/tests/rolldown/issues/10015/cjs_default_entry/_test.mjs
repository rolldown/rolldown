import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

const main = createRequire(import.meta.url)('./dist/main.js');
assert.equal(typeof main, 'function');
assert.equal(await main(), true);
assert.equal(await main(), true);
assert.equal(globalThis.sharedInitializations, 1);
