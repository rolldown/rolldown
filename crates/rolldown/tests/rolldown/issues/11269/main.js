import assert from 'node:assert';
import { Thing, direct } from './lib/index.js';

// `thing.js` and `direct.js` have no module syntax of their own and are only reached through
// `require()`, so they are CommonJS and their top-level `this` is their exports object.
assert.strictEqual(typeof Thing, 'function');
assert.strictEqual(direct, 'direct');
