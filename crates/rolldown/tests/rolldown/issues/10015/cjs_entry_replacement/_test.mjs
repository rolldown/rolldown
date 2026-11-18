import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

const main = createRequire(import.meta.url)('./dist/main.js');
assert.deepEqual(Object.keys(main), ['POST']);
assert.equal(await main.POST(), true);
assert.equal(await main.POST(), true);
assert.equal(globalThis.sharedInitializations, 1);
