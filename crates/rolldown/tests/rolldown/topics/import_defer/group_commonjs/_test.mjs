import assert from 'node:assert';
import { run } from './dist/main.js';

assert.deepStrictEqual(run(), {
  shared: 'shared',
  value: ['shared', 41, 2],
  lazy: 'lazy',
  events: ['shared', 'c', 'e', 'd', 'helper', 'route', 'lazy'],
});
