const assert = require('assert/strict');
const config = require('./config.json');

assert.deepEqual(config, { foo: 1 });
