import assert from "node:assert";

const { used } = require('./lib.js');

assert.equal(used, 'used-value');
