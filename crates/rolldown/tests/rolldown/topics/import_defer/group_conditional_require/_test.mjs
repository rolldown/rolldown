import assert from 'node:assert';
import { run } from './dist/main.js';

assert.deepStrictEqual(run(), { a: 0, before: ['helper'], b: 42, after: ['helper', 'lazy'] });
