import * as assert from "node:assert";
const fn = require('./foo')
assert.equal(fn(), 123)
