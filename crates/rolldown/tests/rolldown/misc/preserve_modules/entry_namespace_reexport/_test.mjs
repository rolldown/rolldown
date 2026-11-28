import assert from 'node:assert/strict';

import * as counterModule from './dist/counter.js';
import { counter } from './dist/main.js';

assert.deepEqual(Object.keys(counterModule), ['count', 'increment']);
assert.equal(counter, counterModule);

counterModule.increment();
assert.equal(counter.count, 1);
