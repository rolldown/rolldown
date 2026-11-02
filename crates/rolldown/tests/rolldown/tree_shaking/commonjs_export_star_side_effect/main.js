import assert from 'node:assert';
import { esm } from './mixed_barrel.js';
import './star_only.js';

assert.strictEqual(esm, 1);
assert.deepStrictEqual(globalThis.evaluated, ['mixed_barrel_dep', 'star_only_dep']);
