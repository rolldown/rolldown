import assert from 'node:assert';
import { run } from './dist/main.js';

assert.deepStrictEqual(run(), [
  42,
  '__rolldown_deferred_group_body__;',
  '\n__rolldown_deferred_group_body__;\n',
]);
