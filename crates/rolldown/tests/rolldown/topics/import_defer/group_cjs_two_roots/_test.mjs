import assert from 'node:assert';
import { run } from './dist/main.js';

assert.deepStrictEqual(run(), {
  before: [],
  first: ['a', 'shared'],
  middle: ['a', 'shared', 'first'],
  second: ['b', 'shared'],
  events: ['a', 'shared', 'first', 'b', 'second'],
});
