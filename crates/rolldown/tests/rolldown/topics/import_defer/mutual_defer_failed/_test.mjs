import assert from 'node:assert';
import { log } from './dist/main.js';

assert.deepStrictEqual(log, ['a', 'b throws', 'b: Error: B failed', 'a: ok']);
