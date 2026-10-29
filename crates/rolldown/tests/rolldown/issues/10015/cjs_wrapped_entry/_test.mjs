import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const consume = require('./dist/consumer.js');
const main = require('./dist/main.js');
assert.deepEqual(Object.keys(main), ['POST']);
assert.equal(consume(), main);
assert.equal(await main.POST(), true);
assert.equal(await main.POST(), true);
assert.equal(globalThis.sharedInitializations, 1);
