import assert from 'node:assert';
import { run } from './dist/main.js';

assert.deepStrictEqual(run(), {
  before: ['helper', 'root'],
  value: ['h', 'inner'],
  after: ['helper', 'root', 'inner'],
});
