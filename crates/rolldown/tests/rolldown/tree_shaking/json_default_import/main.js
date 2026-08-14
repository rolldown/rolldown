import assert from 'node:assert/strict';
import './mutator.js';
import mod from './foo.json';
import mod2 from './bailout.json';
import mutated from './reexport-mutation.json';

assert.strictEqual(mod.a, 'used');
assert.strictEqual(mod2.a, 'bailout_a');
assert.deepEqual(mod2, {
  a: 'bailout_a',
  b: 'bailout_b',
});
assert.strictEqual(mutated.a, 'mutated');
