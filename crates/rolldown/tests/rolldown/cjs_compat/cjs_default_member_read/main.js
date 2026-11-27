import assert from 'node:assert';
import cjs from './legacy.cjs';

assert.strictEqual(cjs.default.x, 'inner');
