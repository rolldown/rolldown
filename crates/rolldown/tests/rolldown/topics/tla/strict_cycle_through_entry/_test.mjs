import assert from 'node:assert';
import { log } from './dist/main.js';

assert.deepStrictEqual(log, ['b', 'a start', 'a end', 'main']);
