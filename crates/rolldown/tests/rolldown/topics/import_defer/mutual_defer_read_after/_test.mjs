import assert from 'node:assert';
import { log } from './dist/main.js';

assert.deepStrictEqual(log, ['a', 'b start', 'b reads a: ok', 'main reads b: ok']);
