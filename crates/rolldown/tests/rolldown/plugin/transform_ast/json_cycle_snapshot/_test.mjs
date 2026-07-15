import assert from 'node:assert/strict';
import { afterMutation, beforeMutation, data } from './dist/main.js';

assert.equal(globalThis.jsonCycleReadBeforePayload, undefined);
assert.equal(beforeMutation, 4);
assert.equal(afterMutation, 4);
assert.equal(data.normal, 9);
