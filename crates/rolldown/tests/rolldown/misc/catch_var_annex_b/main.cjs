const assert = require('node:assert/strict');
const run = require('./dep.cjs');

assert.deepStrictEqual(run(), [[2, 'number'], 'function']);
