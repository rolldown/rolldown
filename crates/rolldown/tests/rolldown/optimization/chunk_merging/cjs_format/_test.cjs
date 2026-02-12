const assert = require("node:assert");
const dep = require("./dist/dep.js");
const main = require("./dist/main.js");

if (globalThis.__configName == "named") {
  assert.deepEqual(main, { [Symbol.toStringTag]: 'Module', value: 42 });
  assert.deepEqual(dep, { [Symbol.toStringTag]: 'Module', default: 42 });
} else {
  assert.deepEqual(main, { [Symbol.toStringTag]: 'Module', value: 42 });
  assert.deepEqual(dep, 42);
}
