import assert from 'node:assert';
import { cn } from './barrel/index.js';

export const ready = import('./route.js').then((route) => {
  assert.strictEqual(cn('a', 'b'), 'a b');
  assert.deepStrictEqual(route.value, { value: 1 });
});
