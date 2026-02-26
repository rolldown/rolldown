import fs from 'node:fs';
import assert from 'node:assert';
import path from 'path';

const file = fs.readFileSync(path.resolve(import.meta.dirname, './dist/main.js'), 'utf-8');

assert.ok(file.includes('obj["cjs-a"] = 1;'));
assert.ok(file.includes('obj["cjs-a"] = 2;'));
assert.equal(file.split('assert.equal("cjs-a", "cjs-a");').length - 1, 2);
