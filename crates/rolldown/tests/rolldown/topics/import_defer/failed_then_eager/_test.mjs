import assert from 'node:assert';
import { done, log } from './dist/main.js';

await done;
assert.deepStrictEqual(log, ['main: Error: X failed', 'j throws: X failed']);
