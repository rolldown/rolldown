import assert from 'node:assert';
import * as path from 'node:path';
import { join, pathNs, sep } from './barrel.js';
import * as barrel from './barrel.js';

assert.strictEqual(join, path.join);
assert.strictEqual(sep, path.sep);
assert.strictEqual(pathNs.join, path.join);
assert.strictEqual(barrel.join, path.join);
assert.deepStrictEqual(Object.keys(barrel), ['join', 'pathNs', 'sep']);
