import assert from 'node:assert';
import { foo } from './reexporter.js';
assert.strictEqual(foo, 'from-cjs');
