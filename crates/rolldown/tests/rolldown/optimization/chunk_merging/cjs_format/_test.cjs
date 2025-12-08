const assert = require("node:assert");
const dep = require("./dist/dep.js");
const main = require("./dist/main.js");

if (globalThis.__configName == "named") {
  assert.deepEqual(main, { value: 42 });
  assert.deepEqual(dep, { default: 42 });
} else {
  assert.deepEqual(main, { value: 42 });
  assert.deepEqual(dep, 42);
}
