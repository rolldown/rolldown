import assert from 'node:assert';
import { bar } from './dist/main.js';

assert.strictEqual(typeof bar, 'function');
