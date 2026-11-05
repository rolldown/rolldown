import assert from 'node:assert';
import { log } from './dist/main.js';

assert.deepStrictEqual(log, ['t', 'e', 'x reads e: e', 'd', 'main reads d: d']);
