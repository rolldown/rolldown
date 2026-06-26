import assert from 'node:assert/strict';

const evalData = require('./eval.json');
assert.strictEqual(evalData.mode, 'eval');
eval('assert.strictEqual(evalData.other, "visible to eval")');
