import assert from 'node:assert';
import fsDef from 'node:fs';
import { readFileSync } from 'node:fs';
import { rfs, fsDefault } from './lib.js';

assert.strictEqual(rfs, readFileSync);
assert.strictEqual(fsDefault, fsDef);

export { readFileSync } from 'node:fs';
