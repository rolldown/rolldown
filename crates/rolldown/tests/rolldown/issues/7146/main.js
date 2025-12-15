const assert = require("assert");
const config = require("./config.json");

assert.deepEqual(config, { foo: 1 });
