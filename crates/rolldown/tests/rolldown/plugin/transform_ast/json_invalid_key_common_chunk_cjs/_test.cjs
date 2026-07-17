const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const a = require('./dist/a.js');
const b = require('./dist/b.js');

assert.equal(a['😈'], 1);
assert.equal(b['property-name'], 2);
assert.equal(a["single'quote"], 3);
assert.equal(a['line\nbreak'], 4);
assert.equal(a['back\\slash'], 5);
assert.deepEqual(a.__proto__, { safe: true });
assert.equal(Object.prototype.hasOwnProperty.call(a, '__proto__'), true);
assert.equal(Object.getPrototypeOf(a), Object.prototype);
assert.equal(globalThis.jsonCommonChunkSideEffectRuns, 1);

const output = fs
  .readdirSync(path.join(__dirname, 'dist'))
  .filter((file) => file.endsWith('.js'))
  .map((file) => fs.readFileSync(path.join(__dirname, 'dist', file), 'utf8'))
  .join('\n');

assert.match(output, /\[["']😈["']\]/);
assert.match(output, /\[["']property-name["']\]/);
assert.ok(!output.includes('.😈'));
assert.ok(!output.includes('.property-name'));
assert.ok(!output.includes("'single'quote'"));
assert.ok(!output.includes('"line\nbreak"'));
