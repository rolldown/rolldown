import assert from 'node:assert';
import { log } from './dist/main.js';

assert.deepStrictEqual(log, ['body', 'disposed']);
