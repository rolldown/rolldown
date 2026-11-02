import assert from 'node:assert';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const mod = require('./dist/main.js');

assert.strictEqual(mod.namedReadFile, require('node:fs').readFile);
assert.strictEqual(mod.readFileSync, require('node:fs').readFileSync);
// Copied values stay redefinable, so `jest.spyOn`/`sinon.stub` work on them.
Object.defineProperty(mod, 'readFileSync', { value: 1 });
assert.strictEqual(mod.readFileSync, 1);
