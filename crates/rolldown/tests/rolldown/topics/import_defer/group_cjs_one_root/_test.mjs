import assert from 'node:assert';
import { run } from './dist/main.js';

assert.deepStrictEqual(run(), { before: [], value: ['a', 'b'], events: ['a', 'b', 'route'] });
