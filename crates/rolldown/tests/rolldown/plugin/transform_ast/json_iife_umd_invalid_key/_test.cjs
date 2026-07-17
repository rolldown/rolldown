const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');

const source = fs.readFileSync(path.join(__dirname, 'dist', 'main.js'), 'utf8');
const sandbox = {};
sandbox.globalThis = sandbox;
vm.runInNewContext(source, sandbox);

const output = sandbox.JsonExports;
assert.equal(sandbox.jsonIifeUmdSideEffectRan, true);
assert.equal(output["single'quote"], 1);
assert.equal(output['line\nbreak'], 2);
assert.equal(output['back\\slash'], 3);
assert.deepEqual({ ...output.__proto__ }, { safe: true });
assert.equal(Object.prototype.hasOwnProperty.call(output, '__proto__'), true);
assert.ok(!source.includes("'single'quote'"));
assert.ok(!source.includes('"line\nbreak"'));
