import assert from 'node:assert';
import { s } from './shared.js';

let m;
try {
  throw 0;
} catch {
  m = await import('./dyn.js');
}
assert.deepStrictEqual([s.value, m.d], [1, 2]);
